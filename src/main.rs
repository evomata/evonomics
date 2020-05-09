use std::time::{Duration, Instant};

// more GUI logic, adapted from iced examples
mod grid;

use iced::{
    button, executor, slider, time, Align, Application, Button, Column, Command, Element,
    HorizontalAlignment, Length, Row, Settings, Slider, Space, Subscription, Text,
    VerticalAlignment,
};

pub fn main() {
    EvonomicsWorld::run(Settings {
        antialiasing: true,
        ..Settings::default()
    })
}

#[derive(Default)]
struct EvonomicsWorld {
    grid: grid::Grid,
    run_simulation_button: button::State,
    load_save_button: button::State,
    save_simulation_button: button::State,
    toggle_run_button: button::State,
    halt_sim_button: button::State,
    toggle_grid_button: button::State,
    speed_slider: slider::State,
    menu_state: MenuState,
    is_running_sim: bool,
    // 1k/speed = number of ms to delay before queuing ticks
    speed: usize,
    // how many ticks the grid is supposed to do (async)
    queued_ticks: usize,
    next_speed: Option<usize>,
}
enum MenuState {
    MainMenu,
    SimMenu,
}
impl std::default::Default for MenuState {
    fn default() -> MenuState {
        MenuState::MainMenu
    }
}

#[derive(Debug, Clone)]
enum MessageType {
    SimView,
    MainView,
    SpeedChanged(f32),
    ToggleSim,
    ToggleGrid,
    Tick(Instant),
    Grid(grid::Message),
}

impl<'a> Application for EvonomicsWorld {
    // application produced messages
    type Message = MessageType;
    // run commands and subscriptions
    type Executor = executor::Default;
    // initialization data for application
    type Flags = ();

    fn new(_flags: ()) -> (EvonomicsWorld, Command<Self::Message>) {
        (
            EvonomicsWorld {
                menu_state: MenuState::MainMenu,
                speed: 16,
                ..EvonomicsWorld::default()
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("Evonomics")
    }

    // handles user interactions
    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            MessageType::SimView => {
                self.menu_state = MenuState::SimMenu;
                self.is_running_sim = true;
            }
            MessageType::MainView => {
                self.menu_state = MenuState::MainMenu;
                self.is_running_sim = false;
            }
            MessageType::SpeedChanged(new_speed) => {
                if self.is_running_sim {
                    self.next_speed = Some(new_speed.round() as usize);
                } else {
                    self.speed = new_speed.round() as usize;
                }
            }
            MessageType::ToggleSim => {
                self.is_running_sim = !self.is_running_sim;
            }
            MessageType::ToggleGrid => {
                self.grid.toggle_lines();
            }
            MessageType::Tick(_) => {
                self.queued_ticks = (self.queued_ticks + 1).min(self.speed);
                if let Some(task) = self.grid.tick(self.queued_ticks) {
                    if let Some(speed) = self.next_speed.take() {
                        self.speed = speed;
                    }
                    self.queued_ticks = 0;
                    return Command::perform(task, MessageType::Grid);
                }
            }
            MessageType::Grid(grid_message) => {
                self.grid.update(grid_message);
            }
        }
        Command::none()
    }

    // queue tick in update function regularly
    fn subscription(&self) -> Subscription<MessageType> {
        if self.is_running_sim {
            time::every(Duration::from_millis(1000 / self.speed as u64)).map(MessageType::Tick)
        } else {
            Subscription::none()
        }
    }

    fn view(&mut self) -> Element<Self::Message> {
        const BUTTON_SIZE: u32 = 200;
        let speed = self.next_speed.unwrap_or(self.speed);
        match self.menu_state {
            MenuState::MainMenu => {
                Column::new()
                    .height(Length::Fill)
                    .width(Length::Fill)
                    .padding(100)
                    .spacing(60)
                    .align_items(Align::Center)
                    .push(Text::new("Evonomics").size(50))
                    .push(
                        Button::new(
                            &mut self.run_simulation_button,
                            Text::new("Run Simulation")
                                .horizontal_alignment(HorizontalAlignment::Center),
                        )
                        .min_width(BUTTON_SIZE)
                        .on_press(MessageType::SimView),
                    )
                    .push(
                        Button::new(
                            &mut self.load_save_button,
                            Text::new("Load Save")
                                .horizontal_alignment(HorizontalAlignment::Center),
                        )
                        .min_width(BUTTON_SIZE),
                    )
                    .into()
                // TODO: .push(settings:labels&radios&sliders) resource list with scarcity sliders, radio button for market entity, radio button for distance trading, slider for trade penalty, slider for carry capacities, slider for barter penalty
            }
            MenuState::SimMenu => {
                Row::new()
                    .push( Row::new().padding(10)
                        .push(
                            Box::new( Column::new().spacing(10).max_width(220)
                                                        .push( Button::new( &mut self.save_simulation_button, Text::new("save") ).min_width(BUTTON_SIZE) )
                                                        .push( Button::new( &mut self.toggle_run_button, if self.is_running_sim { Text::new("Pause") } else { Text::new("Run") } ).min_width(BUTTON_SIZE)
                                                            .on_press( MessageType::ToggleSim ) ) )
                            .push( Slider::new( &mut self.speed_slider, 1.0..=100.0, speed as f32, MessageType::SpeedChanged ) )
                            .push( Text::new(format!("{} Ticks/s", speed) ).size(16).vertical_alignment(VerticalAlignment::Bottom).horizontal_alignment(HorizontalAlignment::Center).width(Length::Fill) )
                            .push( Space::new(Length::Fill, Length::Shrink) )
                            .push( Button::new( &mut self.toggle_grid_button, Text::new("Toggle Grid") ).min_width(BUTTON_SIZE)
                                .on_press( MessageType::ToggleGrid ) )
                            .push( Button::new( &mut self.halt_sim_button, Text::new("Main Menu (Will Pause)") ).min_width(BUTTON_SIZE)
                                .on_press( MessageType::MainView ) )
                            .push( Text::new("Click a cell to see its genome or save it.\n\nClick an empty spot to plant a cell from the save files.\n\nUse the wheel to zoom | right click to pan.") ) )
                            // TODO, requires tracking number of marked ancestors in EvonomicsWorld: .push( table with rows of cell ancestors, collumns of color, hide/show radio button, delete button )
                        .push( self.grid.view().map(MessageType::Grid) ) )
                    .into()
            }
        }
    }
}
