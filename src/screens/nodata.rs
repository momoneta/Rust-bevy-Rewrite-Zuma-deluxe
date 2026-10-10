//! Shown when the original game folder cannot be found.

use bevy::prelude::*;

pub const HELP: &str = "Zuma Deluxe data not found.\n\n\
This is an engine rewrite: it needs the files of your own copy of\n\
Zuma Deluxe (folders images/, levels/, sounds/, fonts/, properties/).\n\n\
Copy the \"Zuma Deluxe\" folder next to the executable and rename it\n\
to \"data\", or start the game with:\n\n\
    zuma --data \"/path/to/Zuma Deluxe\"\n\n\
or set the ZUMA_DATA environment variable.";

pub fn enter(mut commands: Commands) {
    error!("{HELP}");
    commands.spawn((
        Text2d::new(HELP),
        TextFont { font_size: FontSize::Px(14.0), ..default() },
        TextColor(Color::srgb(1.0, 0.9, 0.6)),
        TextLayout::justify(Justify::Center),
    ));
}
