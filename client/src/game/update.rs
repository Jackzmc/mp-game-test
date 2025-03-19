use macroquad::input::{is_key_pressed, is_key_released, is_mouse_button_down, set_cursor_grab, KeyCode, MouseButton};
use macroquad::logging::debug;
use macroquad::math::{vec3, Vec3};
use mp_game_test_common::game::{Action, PlayerData};
use crate::game::GameInstance;
use crate::{get_direction_vector, ActionResult};

impl GameInstance {
    pub fn update(&mut self) {
        // Process incoming net data
        // if let Some(event) = self.net_mut().next_event() {
        //     macroquad::logging::debug!("[main->loop] got event, processing: {:?}", event);
        //     self.process_event(event);
        // }

        // Processes all pending incoming net data
        while let Some(event) = self.net_mut().next_event() {
            macroquad::logging::debug!("[main->loop] got event, processing: {:?}", event);
            self.process_event(event);
        }

        set_action(self, Action::Forward, KeyCode::W);
        set_action(self, Action::Backward, KeyCode::S);

        set_action(self, Action::Left, KeyCode::A);
        set_action(self, Action::Right, KeyCode::D);

        self.process_player();
    }

    fn process_player(&mut self) {
        if self.player_mut().is_none() {
            return;
        }
        let player = self.player_mut().unwrap();

        // cam.offset = Vec2::new(player.position.x, player.position.y);
        // trace!("cam.offset={}", cam.offset);

        if is_mouse_button_down(MouseButton::Left) {
            player.angles.z += 0.01;


            // let mouse_pos = mouse_position_local();
            // let mouse_delta = mouse_pos - *last_mouse_pos;
            // *last_mouse_pos = mouse_pos;
            // let time_delta = time_delta.as_secs_f32();

            // game.cam.rotation.x += mouse_delta.x * time_delta * 1.0;
            // game.cam.rotation.y += mouse_delta.y * time_delta * -1.0;
            // game.cam.rotation.y = clamp(game.cam.rotation.y, -1.5, 1.5);

            set_cursor_grab(true);
            // let pos = vec3(player.position.x, player.position.y, player.position.z);
            // game.cam.set_target(pos);
        } else if is_mouse_button_down(MouseButton::Right) {
            player.angles.x -= 0.01;
        } else {
            set_cursor_grab(false);
        }
        let dist_cam = 8.0;
        let player_pos = vec3(player.position.x, player.position.y, player.position.z);
        let ang = vec3(player.angles.x, player.angles.y, player.angles.z);
        let dir_vec = get_direction_vector(&Vec3 {x: 0.0, y: -dist_cam, z: 0.0}, &ang);
        self.cam.camera.position = player_pos + dir_vec;
        self.cam.camera.target = vec3(player_pos.x, player_pos.y, 2.0);
        debug!("cam_pos {:?}\tcam_tar {:?}\tang {:?}", self.cam.camera.position, player_pos, ang);
    }
}

fn set_action(game: &mut GameInstance, action: Action, key_code: KeyCode) -> ActionResult {
    if is_key_pressed(key_code) {
        game.update_action(action, true).ok();
        ActionResult::Activated
    } else if game.has_action(action) && is_key_released(key_code) {
        game.update_action(action, false).ok();
        ActionResult::Deactivated
    } else {
        ActionResult::None
    }
}

