use iced::{ Background, container, Color, button, slider };

pub struct Theme {}

pub const MAIN_MENU_COLLUMN_WIDTH: u32 = 350;
pub const BUTTON_SIZE: u32 = 200;
pub const PADDING: u16 = 10;
pub const SPACING: u16 = 20;

macro_rules! color_const {
    ( $r:expr, $g:expr, $b:expr ) => {
        Color { r: $r as f32 / 255.0, g: $g as f32 / 255.0, b: $b as f32 / 255.0, a: 1.0 }
    };
}

pub const COLOR_GOLD: Color = color_const!( 0xD4, 0xAF, 0x37 );
pub const COLOR_RHODIUM: Color = color_const!( 0xE2, 0xE7, 0xE1 );
pub const COLOR_PLATINUM: Color = color_const!( 0xE5, 0xE4, 0xE2 );
pub const COLOR_PALLADIUM: Color = color_const!( 0x6F, 0x6A, 0x75 );
pub const COLOR_OSMIUM: Color = color_const!( 0x90, 0x90, 0xA3 );

pub struct Slider;
impl slider::StyleSheet for Slider {
    fn active(&self) -> slider::Style {
        slider::Style {
            rail_colors: (COLOR_GOLD, Color { a: 0.1, ..COLOR_GOLD }),
            handle: slider::Handle {
                shape: slider::HandleShape::Circle { radius: 9 },
                color: COLOR_GOLD,
                border_width: 0,
                border_color: Color::TRANSPARENT,
            },
        }
    }
    fn hovered(&self) -> slider::Style {
        let active = self.active();

        slider::Style {
            handle: slider::Handle {
                color: COLOR_PLATINUM,
                ..active.handle
            },
            ..active
        }
    }
    fn dragging(&self) -> slider::Style {
        let active = self.active();

        slider::Style {
            handle: slider::Handle {
                color: COLOR_RHODIUM,
                ..active.handle
            },
            ..active
        }
    }
}
impl From<Theme> for Box<dyn slider::StyleSheet> {
    fn from(_: Theme) -> Self {
        Slider.into()
    }
}

pub struct Container;
impl container::StyleSheet for Container {
    fn style(&self) -> container::Style {
        container::Style {
            background: Some(Background::Color(COLOR_PALLADIUM)),
            text_color: Some(COLOR_GOLD),
            ..container::Style::default()
        }
    }
}
impl From<Theme> for Box<dyn container::StyleSheet> {
    fn from(_: Theme) -> Self {
        Container.into()
    }
}

pub struct Button;
impl button::StyleSheet for Button {
    fn active(&self) -> button::Style {
        button::Style {
            background: Some(Background::Color(COLOR_OSMIUM)),
            text_color: COLOR_GOLD,
            ..button::Style::default()
        }
    }
    fn hovered(&self) -> button::Style {
        button::Style {
            text_color: Color::WHITE,
            ..self.active()
        }
    }
}
impl From<Theme> for Box<dyn button::StyleSheet> {
    fn from(_: Theme) -> Self {
        Button.into()
    }
}