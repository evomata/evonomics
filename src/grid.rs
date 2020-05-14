use crate::sim;
use float_ord::FloatOrd;
use std::collections::VecDeque;
use std::ops::RangeInclusive;
use std::time::{Duration, Instant};

use iced::{
    canvas::{self, Cache, Canvas, Cursor, Event, Frame, Geometry, Path},
    mouse, Color, Element, Length, Point, Rectangle, Size, Vector,
};

const CELL_SIZE: usize = 20;
const MAX_SCALING: f32 = 2.0;

const AVERAGING_COUNT: usize = 15;

#[derive(Debug)]
pub enum Message {
    View(sim::View),
}

impl From<sim::View> for Message {
    fn from(view: sim::View) -> Self {
        Self::View(view)
    }
}

pub struct Grid {
    min_scaling: f32,
    view: sim::View,
    interaction: Interaction,
    life_cache: Cache,
    grid_cache: Cache,
    translation: Vector,
    scaling: f32,
    show_lines: bool,
    tick_durations: VecDeque<(Duration, usize)>,
    /// When a tick comes in, this is used to measure the elapsed time of the tick.
    tick_start: Instant,
}

impl Grid {
    pub fn new(width: usize, height: usize) -> Self {
        let initial_x: f32 = -((CELL_SIZE * width) as f32) * 0.5;
        let initial_y: f32 = -((CELL_SIZE * height) as f32) * 0.5;
        Self {
            min_scaling: 0.105 * 512.0 / width as f32,
            view: sim::View::default(),
            interaction: Interaction::None,
            life_cache: Cache::default(),
            grid_cache: Cache::default(),
            translation: Vector::new(initial_x, initial_y),
            scaling: 1.0,
            show_lines: false,
            tick_durations: vec![].into(),
            tick_start: Instant::now(),
        }
    }

    pub fn get_ticks_per_second (&self) -> f64 {
        let val = self
            .tick_durations
            .iter()
            .fold( (0f64, 0), |acc, (duration, ticks)| (duration.as_secs_f64() + acc.0, ticks + acc.1) );
        val.1 as f64 / val.0
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::View(view) => {
                // Replace our old view with this new view.
                self.view = view;
                let tick_duration = self.tick_start.elapsed();
                self.tick_start = Instant::now();
                self.tick_durations.push_front( (tick_duration, self.view.ticks) );
                self.tick_durations.truncate(AVERAGING_COUNT);
                self.life_cache.clear();
            }
        }
    }

    pub fn view<'a>(&'a mut self) -> Element<'a, ()> {
        Canvas::new(self)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn toggle_lines(&mut self) {
        self.show_lines = !self.show_lines;
    }

    // used for grid lines, determining when cells are visible, and accurately placing the mouse
    fn visible_region(&self, size: Size) -> Region {
        let width = size.width / self.scaling;
        let height = size.height / self.scaling;

        Region {
            x: -self.translation.x - width / 2.0,
            y: -self.translation.y - height / 2.0,
            width,
            height,
        }
    }

    fn project(&self, position: Point, size: Size) -> Point {
        let region = self.visible_region(size);

        Point::new(
            position.x / self.scaling + region.x,
            position.y / self.scaling + region.y,
        )
    }
}

impl canvas::Program<()> for Grid {
    fn update(&mut self, event: Event, bounds: Rectangle, cursor: Cursor) -> Option<()> {
        if let Event::Mouse(mouse::Event::ButtonReleased(_)) = event {
            self.interaction = Interaction::None;
        }

        let cursor_position = cursor.position_in(&bounds)?;

        match event {
            Event::Mouse(mouse_event) => match mouse_event {
                mouse::Event::ButtonPressed(button) => match button {
                    // TODO <CELL INTERACTION>
                    // mouse::Button::Left => {
                    //     self.interaction = if is_populated {
                    //         Interaction::Erasing
                    //     } else {
                    //         Interaction::Drawing
                    //     };
                    //     populate.or(unpopulate)
                    // }
                    mouse::Button::Right => {
                        self.interaction = Interaction::Panning {
                            translation: self.translation,
                            start: cursor_position,
                        };

                        None
                    }
                    _ => None,
                },
                mouse::Event::CursorMoved { .. } => {
                    match self.interaction {
                        // TODO <CELL INTERACTION>
                        // Interaction::Drawing => populate,
                        // Interaction::Erasing => unpopulate,
                        Interaction::Panning { translation, start } => {
                            self.translation =
                                translation + (cursor_position - start) * (1.0 / self.scaling);

                            self.life_cache.clear();
                            self.grid_cache.clear();

                            None
                        }
                        _ => None,
                    }
                }
                mouse::Event::WheelScrolled { delta } => match delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                        if y < 0.0 && self.scaling > self.min_scaling
                            || y > 0.0 && self.scaling < MAX_SCALING
                        {
                            let old_scaling = self.scaling;

                            self.scaling = (self.scaling * (1.0 + y / 30.0))
                                .max(self.min_scaling)
                                .min(MAX_SCALING);

                            if let Some(cursor_to_center) = cursor.position_from(bounds.center()) {
                                let factor = self.scaling - old_scaling;

                                self.translation = self.translation
                                    - Vector::new(
                                        cursor_to_center.x * factor / (old_scaling * old_scaling),
                                        cursor_to_center.y * factor / (old_scaling * old_scaling),
                                    );
                            }

                            self.life_cache.clear();
                            self.grid_cache.clear();
                        }

                        None
                    }
                },
                _ => None,
            },
        }
    }

    fn draw(&self, bounds: Rectangle, cursor: Cursor) -> Vec<Geometry> {
        let center = Vector::new(bounds.width / 2.0, bounds.height / 2.0);

        let life = self.life_cache.draw(bounds.size(), |frame| {
            let background = Path::rectangle(Point::ORIGIN, frame.size());
            frame.fill(&background, Color::from_rgb8(0x40, 0x44, 0x4B));

            frame.with_save(|frame| {
                frame.translate(center);
                frame.scale(self.scaling);
                frame.translate(self.translation);
                frame.scale(CELL_SIZE as f32);

                let region = self.visible_region(frame.size());

                for ((y, x), &color) in self.view.colors.indexed_iter() {
                    if region.contained(x, y) {
                        frame.fill_rectangle(Point::new(x as f32, y as f32), Size::UNIT, color);
                    }
                }
            });
        });

        let overlay = {
            let mut frame = Frame::new(bounds.size());

            let hovered_cell = cursor.position_in(&bounds).map(|position| {
                let point = self.project(position, frame.size());
                cell_at(point.x, point.y)
            });

            if let Some(cell) = hovered_cell {
                frame.with_save(|frame| {
                    frame.translate(center);
                    frame.scale(self.scaling);
                    frame.translate(self.translation);
                    frame.scale(CELL_SIZE as f32);

                    frame.fill_rectangle(
                        Point::new(cell.0 as f32, cell.1 as f32),
                        Size::UNIT,
                        Color {
                            a: 0.5,
                            ..Color::BLACK
                        },
                    );
                });
            }

            frame.into_geometry()
        };

        if self.scaling < 0.2 || !self.show_lines {
            vec![life, overlay]
        } else {
            let grid = self.grid_cache.draw(bounds.size(), |frame| {
                frame.translate(center);
                frame.scale(self.scaling);
                frame.translate(self.translation);
                frame.scale(CELL_SIZE as f32);

                let region = self.visible_region(frame.size());
                let rows = region.rows();
                let columns = region.columns();
                let (total_rows, total_columns) = (rows.clone().count(), columns.clone().count());
                let width = 2.0 / CELL_SIZE as f32;
                let color = Color::from_rgb8(70, 74, 83);

                frame.translate(Vector::new(-width / 2.0, -width / 2.0));

                for row in region.rows() {
                    frame.fill_rectangle(
                        Point::new(*columns.start() as f32, row as f32),
                        Size::new(total_columns as f32, width),
                        color,
                    );
                }

                for column in region.columns() {
                    frame.fill_rectangle(
                        Point::new(column as f32, *rows.start() as f32),
                        Size::new(width, total_rows as f32),
                        color,
                    );
                }
            });

            vec![life, grid, overlay]
        }
    }

    fn mouse_interaction(&self, bounds: Rectangle, cursor: Cursor) -> mouse::Interaction {
        match self.interaction {
            // Interaction::Drawing => mouse::Interaction::Crosshair,
            // Interaction::Erasing => mouse::Interaction::Crosshair,
            Interaction::Panning { .. } => mouse::Interaction::Grabbing,
            Interaction::None if cursor.is_over(&bounds) => mouse::Interaction::Crosshair,
            _ => mouse::Interaction::default(),
        }
    }
}

pub struct Region {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

fn lim_0(n: f32) -> f32 {
    std::cmp::max(FloatOrd(0.0), FloatOrd(n)).0
}

impl Region {
    fn rows(&self) -> RangeInclusive<usize> {
        let first_row = lim_0((self.y / CELL_SIZE as f32).floor()) as usize;

        let visible_rows = lim_0((self.height / CELL_SIZE as f32).ceil()) as usize;

        first_row..=first_row + visible_rows
    }

    fn columns(&self) -> RangeInclusive<usize> {
        let first_column = lim_0((self.x / CELL_SIZE as f32).floor()) as usize;

        let visible_columns = lim_0((self.width / CELL_SIZE as f32).ceil()) as usize;

        first_column..=first_column + visible_columns
    }

    fn contained(&self, _i: usize, _j: usize) -> bool {
        // self.rows().contains(&i) && self.columns().contains(&j)
        // FIXME
        true
    }
}

enum Interaction {
    None,
    // Drawing,
    // Erasing,
    Panning { translation: Vector, start: Point },
}

pub fn cell_at(x: f32, y: f32) -> (isize, isize) {
    (
        (x.ceil() as isize).saturating_sub(1) / CELL_SIZE as isize,
        (y.ceil() as isize).saturating_sub(1) / CELL_SIZE as isize,
    )
}
