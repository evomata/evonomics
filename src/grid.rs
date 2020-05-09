// Grid code modified from ICED Examples
// MIT

mod evo;

use std::future::Future;
use std::ops::RangeInclusive;
use std::time::{Duration, Instant};

use iced::{
    canvas::{self, Cache, Canvas, Cursor, Event, Frame, Geometry, Path, Text},
    mouse, Color, Element, HorizontalAlignment, Length, Point, Rectangle, Size, Vector,
    VerticalAlignment,
};

const AVERAGING_COUNT: usize = 15;

pub struct Grid {
    state: evo::State,
    interaction: Interaction,
    life_cache: Cache,
    grid_cache: Cache,
    translation: Vector,
    scaling: f32,
    show_lines: bool,
    last_tick_duration: [u128; AVERAGING_COUNT],
    inter_tick_duration: [f32; AVERAGING_COUNT],
    last_tick_start: Instant,
    last_queued_ticks: usize,
    version: usize,
}

#[derive(Debug, Clone)]
pub enum Message {
    // Populate(evo::CellState),
    // Unpopulate(evo::CellState),
    Ticked {
        result: Result<evo::LifeContainer, evo::TickError>,
        tick_duration: Duration,
        version: usize,
    },
}

impl<'a> Default for Grid {
    fn default() -> Self {
        Self {
            state: evo::State::default(),
            interaction: Interaction::None,
            life_cache: Cache::default(),
            grid_cache: Cache::default(),
            translation: Vector::new(Self::INITIAL_POS, Self::INITIAL_POS),
            scaling: 1.0,
            show_lines: true,
            last_tick_duration: Default::default(),
            inter_tick_duration: Default::default(),
            last_tick_start: Instant::now(),
            last_queued_ticks: 0,
            version: 0,
        }
    }
}

impl Grid {
    const MIN_SCALING: f32 = 0.1;
    const MAX_SCALING: f32 = 2.0;
    const INITIAL_POS: f32 = -((evo::CellState::SIZE * evo::State::SIDE) as f32) * 0.5;

    pub fn tick(&mut self, amount: usize) -> Option<impl Future<Output = Message>> {
        let version = self.version;
        let tick = self.state.tick(amount)?;

        self.last_queued_ticks = amount;

        Some(async move {
            let start = Instant::now();
            let result = tick.await;
            let tick_duration = start.elapsed() / amount as u32;

            Message::Ticked {
                result,
                version,
                tick_duration,
            }
        })
    }

    pub fn update(&mut self, message: Message) {
        match message {
            // Message::Populate(_cell) => {
            // TODO <CELL INTERACTION>: implement this so that the mouse interaction code provides a cell from an opened menu or simply does not invoke this
            // self.state.populate(cell);
            // self.life_cache.clear();
            // }
            // Message::Unpopulate(_cell) => {
            // TODO <CELL INTERACTION>: instead of an unpopulate message, open cell view for save/genome exam
            // }
            Message::Ticked {
                result: Ok(life),
                version,
                tick_duration,
            } if version == self.version => {
                self.state.update(life);
                self.life_cache.clear();

                for i in 1..AVERAGING_COUNT {
                    let v = AVERAGING_COUNT - i;
                    self.inter_tick_duration[v] = self.inter_tick_duration[v - 1];
                    self.last_tick_duration[v] = self.last_tick_duration[v - 1];
                }
                self.last_tick_duration[0] = tick_duration.as_millis();
                self.inter_tick_duration[0] = self.last_tick_start.elapsed().as_secs_f32();

                self.last_tick_start = Instant::now();
            }
            Message::Ticked {
                result: Err(error), ..
            } => {
                dbg!(error);
            }
            Message::Ticked { .. } => {}
        }
    }

    pub fn view<'a>(&'a mut self) -> Element<'a, Message> {
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

impl canvas::Program<Message> for Grid {
    fn update(&mut self, event: Event, bounds: Rectangle, cursor: Cursor) -> Option<Message> {
        if let Event::Mouse(mouse::Event::ButtonReleased(_)) = event {
            self.interaction = Interaction::None;
        }

        let cursor_position = cursor.position_in(&bounds)?;
        // TODO OKAY; so.. i,j are isize expected by ice; x,y are floating point from mouse position projection; gridsim code uses usize
        //  check where primitive choices for ice and gridsim code are made and if pertinent, change them for more consistency
        //     If not pertinent, at least change these to ensure there are no boundary issues!
        // TODO <CELL INTERACTION>: more complicated gridsim cells won't have enough information for generating CellState cells for these messages
        // let point = self.project( cursor_position, bounds.size() );
        //        let (i, j) = evo::CellState::at( point.x, point.y );
        // let is_populated = self.state.cell_at(i as usize, j as usize);
        //    populate should be re-implemented with the adition of selecting marked cells from the list
        //    unpopulate has been removed; however, there is to be a new behavior shiftinig menu state when an active cell is clicked
        // let (populate, unpopulate) = if is_populated {
        //     (None, Some(Message::Unpopulate(cell)))
        // } else {
        //     (Some(Message::Populate(cell)), None)
        // };

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
                        if y < 0.0 && self.scaling > Self::MIN_SCALING
                            || y > 0.0 && self.scaling < Self::MAX_SCALING
                        {
                            let old_scaling = self.scaling;

                            self.scaling = (self.scaling * (1.0 + y / 30.0))
                                .max(Self::MIN_SCALING)
                                .min(Self::MAX_SCALING);

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
                frame.scale(evo::CellState::SIZE as f32);

                let region = self.visible_region(frame.size());

                self.state
                    .cells()
                    .iter()
                    .enumerate()
                    .for_each(|(ix, cell)| {
                        let (i, j) = self.state.gen_xy_pos(ix);
                        if region.contained(i, j) {
                            frame.fill_rectangle(
                                Point::new(j as f32, i as f32),
                                Size::UNIT,
                                if cell.is_ant() {
                                    Color::from_rgb8(0xFF, 0x0, 0x0)
                                } else if cell.has_color() {
                                    Color::WHITE
                                } else {
                                    Color::from_rgb8(0x48, 0x4C, 0x54)
                                },
                            );
                        }
                    });
            });
        });

        let overlay = {
            let mut frame = Frame::new(bounds.size());

            let hovered_cell = cursor.position_in(&bounds).map(|position| {
                let point = self.project(position, frame.size());
                evo::CellState::at(point.x, point.y)
            });

            if let Some(cell) = hovered_cell {
                frame.with_save(|frame| {
                    frame.translate(center);
                    frame.scale(self.scaling);
                    frame.translate(self.translation);
                    frame.scale(evo::CellState::SIZE as f32);

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

            let text = Text {
                color: Color::from_rgb8(0x10, 0x88, 0x88),
                size: 20.0,
                position: Point::new(frame.width(), frame.height()),
                horizontal_alignment: HorizontalAlignment::Right,
                vertical_alignment: VerticalAlignment::Bottom,
                ..Text::default()
            };

            if let Some(cell) = hovered_cell {
                frame.fill_text(Text {
                    content: format!("({}, {})", cell.0, cell.1),
                    position: text.position - Vector::new(0.0, 16.0),
                    ..text
                });
            }

            let cell_count = self.state.cell_count();

            frame.fill_text(Text {
                content: format!(
                    "{} cell{} @ {} Ms/Tick, {:.3} Ticks/s.. Queued Ticks: {}",
                    cell_count,
                    if cell_count == 1 { "" } else { "s" },
                    self.last_tick_duration.iter().fold(0, |val, dur| val + dur)
                        / AVERAGING_COUNT as u128,
                    AVERAGING_COUNT as f32
                        / self
                            .inter_tick_duration
                            .iter()
                            .fold(0.0, |val, dur| val + dur),
                    self.last_queued_ticks
                ),
                ..text
            });

            frame.into_geometry()
        };

        if self.scaling < 0.2 || !self.show_lines {
            vec![life, overlay]
        } else {
            let grid = self.grid_cache.draw(bounds.size(), |frame| {
                frame.translate(center);
                frame.scale(self.scaling);
                frame.translate(self.translation);
                frame.scale(evo::CellState::SIZE as f32);

                let region = self.visible_region(frame.size());
                let rows = region.rows();
                let columns = region.columns();
                let (total_rows, total_columns) = (rows.clone().count(), columns.clone().count());
                let width = 2.0 / evo::CellState::SIZE as f32;
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

impl Region {
    fn rows(&self) -> RangeInclusive<isize> {
        let first_row = (self.y / evo::CellState::SIZE as f32).floor() as isize;

        let visible_rows = (self.height / evo::CellState::SIZE as f32).ceil() as isize;

        first_row..=first_row + visible_rows
    }

    fn columns(&self) -> RangeInclusive<isize> {
        let first_column = (self.x / evo::CellState::SIZE as f32).floor() as isize;

        let visible_columns = (self.width / evo::CellState::SIZE as f32).ceil() as isize;

        first_column..=first_column + visible_columns
    }

    fn contained(&self, i: isize, j: isize) -> bool {
        self.rows().contains(&i) && self.columns().contains(&j)
    }
}

enum Interaction {
    None,
    // Drawing,
    // Erasing,
    Panning { translation: Vector, start: Point },
}
