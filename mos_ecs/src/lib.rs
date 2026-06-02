#![no_std]

use bevy::DefaultPlugins;
use bevy::app::Startup;
use log::info;

mod test {
    use bevy::app::Startup;
    use log::info;

    #[test]
    fn start_ecs() {
        bevy::app::App::new()
            .add_systems(Startup, || info!("Startup"))
            .run();
    }
}

pub fn test_ecs() {
    bevy::app::App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, || info!("[ECS] Startup"))
        .run();
}
