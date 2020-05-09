mod grid;
pub mod sim;

use futures::{
    channel::mpsc::{Receiver, Sender},
    prelude::*,
};
use iced::{
    button, executor, slider, time, Align, Application, Button, Column, Command, Element,
    HorizontalAlignment, Length, Row, Settings, Slider, Space, Subscription, Text,
    VerticalAlignment,
};
use std::time::Duration;

pub fn main() {
    EvonomicsWorld::run(Settings {
        antialiasing: true,
        ..Settings::default()
    })
}

struct EvonomicsWorld {
    grid: grid::Grid,
    sim_tx: Sender<sim::ToSim>,
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

#[derive(Debug)]
enum Message {
    FromSim(sim::FromSim, Receiver<sim::FromSim>),
    SimView,
    MainView,
    SpeedChanged(f32),
    ToggleSim,
    ToggleGrid,
    Tick,
    Null,
}

impl Clone for Message {
    fn clone(&self) -> Self {
        match self {
            Self::SimView => Self::SimView,
            Self::MainView => Self::MainView,
            Self::ToggleSim => Self::ToggleSim,
            Self::ToggleGrid => Self::ToggleGrid,
            Self::Tick => Self::Tick,
            _ => panic!("do not try to clone messages with data in them"),
        }
    }
}

fn reciever_command(rx: Receiver<sim::FromSim>) -> Command<Message> {
    Command::perform(rx.into_future(), |(item, stream)| {
        Message::FromSim(item.expect("sim_rx ended unexpectedly"), stream)
    })
}

impl<'a> Application for EvonomicsWorld {
    // application produced messages
    type Message = Message;
    // run commands and subscriptions
    type Executor = executor::Default;
    // initialization data for application
    type Flags = ();

    fn new(_: ()) -> (EvonomicsWorld, Command<Self::Message>) {
        let (sim_tx, sim_rx, sim_runner) = sim::run_sim(500, 1);
        (
            EvonomicsWorld {
                grid: Default::default(),
                sim_tx,
                run_simulation_button: Default::default(),
                load_save_button: Default::default(),
                save_simulation_button: Default::default(),
                toggle_run_button: Default::default(),
                halt_sim_button: Default::default(),
                toggle_grid_button: Default::default(),
                speed_slider: Default::default(),
                menu_state: MenuState::MainMenu,
                is_running_sim: false,
                speed: 16,
                next_speed: None,
            },
            Command::batch(vec![
                Command::perform(sim_runner, |_| panic!()),
                reciever_command(sim_rx),
            ]),
        )
    }

    fn title(&self) -> String {
        String::from("Evonomics")
    }

    // handles user interactions
    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::FromSim(from_sim, stream) => {
                match from_sim {
                    sim::FromSim::View(view) => self.grid.update(view.into()),
                }
                return reciever_command(stream);
            }
            Message::SimView => {
                self.menu_state = MenuState::SimMenu;
                self.is_running_sim = true;
            }
            Message::MainView => {
                self.menu_state = MenuState::MainMenu;
                self.is_running_sim = false;
            }
            Message::SpeedChanged(new_speed) => {
                if self.is_running_sim {
                    self.next_speed = Some(new_speed.round() as usize);
                } else {
                    self.speed = new_speed.round() as usize;
                }
            }
            Message::ToggleSim => {
                self.is_running_sim = !self.is_running_sim;
            }
            Message::ToggleGrid => {
                self.grid.toggle_lines();
            }
            Message::Tick => {
                let mut sim_tx = self.sim_tx.clone();
                return Command::perform(
                    async move { sim_tx.send(sim::ToSim::Tick(1)).await },
                    |res| {
                        res.map(|_| Message::Null)
                            .expect("sim_tx ended unexpectedly")
                    },
                );
            }
            Message::Null => {}
        }
        Command::none()
    }

    // queue tick in update function regularly
    fn subscription(&self) -> Subscription<Message> {
        if self.is_running_sim {
            time::every(Duration::from_millis((1000.0 / self.speed as f64) as u64))
                .map(|_| Message::Tick)
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
                        .on_press(Message::SimView),
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
                                                            .on_press( Message::ToggleSim ) ) )
                            .push( Slider::new( &mut self.speed_slider, 1.0..=100.0, speed as f32, Message::SpeedChanged ) )
                            .push( Text::new(format!("{} Ticks/s", speed) ).size(16).vertical_alignment(VerticalAlignment::Bottom).horizontal_alignment(HorizontalAlignment::Center).width(Length::Fill) )
                            .push( Space::new(Length::Fill, Length::Shrink) )
                            .push( Button::new( &mut self.toggle_grid_button, Text::new("Toggle Grid") ).min_width(BUTTON_SIZE)
                                .on_press( Message::ToggleGrid ) )
                            .push( Button::new( &mut self.halt_sim_button, Text::new("Main Menu (Will Pause)") ).min_width(BUTTON_SIZE)
                                .on_press( Message::MainView ) )
                            .push( Text::new("Click a cell to see its genome or save it.\n\nClick an empty spot to plant a cell from the save files.\n\nUse the wheel to zoom | right click to pan.") ) )
                            // TODO, requires tracking number of marked ancestors in EvonomicsWorld: .push( table with rows of cell ancestors, collumns of color, hide/show radio button, delete button )
                        .push( self.grid.view().map(|_| Message::Null) ) )
                    .into()
            }
        }
    }
}
