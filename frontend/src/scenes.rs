use bevy::prelude::*;
use bevy_egui::{
    egui::{self, load::SizedTexture, Align2, Color32, FontId, ImageSource, RichText},
    EguiContexts,
};

// === GameState enum ===
#[derive(States, Clone, Eq, PartialEq, Debug, Hash, Default)]
pub enum GameState {
    #[default]
    Loading,
    RoomCreation,
    Playing,
}

// === Assets ===
#[derive(Resource)]
pub struct Images {
    dog: Handle<Image>,
}

impl FromWorld for Images {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.get_resource_mut::<AssetServer>().unwrap();
        Self {
            dog: asset_server.load("dog.png"),
        }
    }
}

// === Loading scenes ===

pub fn draw_loading_ui(
    mut contexts: EguiContexts,
    mut is_initialized: Local<bool>,
    mut rendered_texture_id: Local<egui::TextureId>,
    images: Res<Images>,
) {
    if !*is_initialized {
        *is_initialized = true;
        *rendered_texture_id = contexts.add_image(images.dog.clone_weak());
    }

    egui::Area::new("example_area2".into())
        .anchor(Align2::CENTER_TOP, (0., 100.))
        .show(contexts.ctx_mut(), |ui| {
            let added_button = ui.add(egui::ImageButton::new(egui::widgets::Image::new(egui::load::SizedTexture::new(
                *rendered_texture_id,
                [256.0, 256.0],
            ))));
            if added_button.clicked() {
                println!("Image clicked!");
            }
        });

    egui::Area::new("score".into())
        .anchor(Align2::CENTER_TOP, (0., 25.))
        .show(contexts.ctx_mut(), |ui| {
            ui.label(
                RichText::new(format!("0 - 2"))
                    .color(Color32::BLACK)
                    .font(FontId::proportional(72.0)),
            );
            let rect = egui::Rect::from_min_size(Default::default(), egui::Vec2::splat(100.0));
            egui::Image::new(egui::include_image!("../assets/dog.png"))
                .rounding(5.0)
                .tint(egui::Color32::LIGHT_BLUE)
                .paint_at(ui, rect);
        });
}

pub fn get_loading_system_methods(
) -> fn(EguiContexts, Local<bool>, Local<egui::TextureId>, Res<Images>) {
    draw_loading_ui
}

// === Intro scenes ===

pub fn draw_intro_ui(mut contexts: EguiContexts) {
    egui::Area::new("example_area".into())
        .anchor(Align2::CENTER_TOP, (0., 25.))
        .show(contexts.ctx_mut(), |ui| {
            if ui.button("Click me").clicked() {
                println!("Button clicked!");
            }
        });
}

pub fn get_intro_system_methods() -> fn(EguiContexts) {
    draw_intro_ui
}
