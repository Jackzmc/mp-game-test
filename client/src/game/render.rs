use macroquad::camera::{set_camera, set_default_camera};
use macroquad::color::{Color, BLACK, DARKGRAY, ORANGE, PURPLE, RED, WHITE};
use macroquad::input::{is_mouse_button_down, set_cursor_grab, KeyCode, MouseButton};
use macroquad::logging::debug;
use macroquad::math::{vec2, vec3, Vec3};
use macroquad::models::{draw_cube, draw_grid, draw_line_3d, draw_mesh, Mesh, Vertex};
use macroquad::prelude::{clear_background, draw_text, measure_text, screen_width};
use mp_game_test_common::def::MAX_PLAYERS;
use mp_game_test_common::game::Action;
use crate::game::GameInstance;
use crate::{get_direction_vector, FpsCounter, Player};

impl GameInstance {
    pub fn render(&mut self) {
        clear_background(WHITE);

        set_camera(&mut self.cam.camera);

        draw_grid(3, 10.0, BLACK, WHITE);

        // let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(10);
        // let mut color = Color::new(0.5, 0.5, 0.5, 1.0);
        let mut mesh = Mesh {
            vertices: vec![
                Vertex::new2(vec3(0.0, 15.0, 0.0),vec2(0.0,0.0),Color::new(1.0,0.0,1.0,1.0)),
                Vertex::new2(vec3(-0.15, -0.15, 0.0),vec2(0.0,0.0),Color::new(0.0,0.0,1.0,1.0)),
                Vertex::new2(vec3(0.15, -0.15 ,0.0),vec2(0.0,0.0),Color::new(0.0,0.0,1.0,1.0)),
            ],
            indices: vec![0,1,2],
            texture: None
        };
        draw_cube(vec3(5.0,5.0,0.0), vec3(1.0,1.0,1.0), None, PURPLE);
        draw_mesh(&mesh);
        // for x in -20..20 {
        //     for y in -20..20 {
        //         let height = rng.random_range(0.0..8.0);
        //         mesh.vertices.push(Vertex {
        //             position: vec3(x as f32, y as f32, height as f32),
        //             color: [200, 0, 0, 255],
        //             uv: vec2(20.0, 0.0),
        //             normal: vec4(20.0, 0.0, 0.0, 0.0),
        //         });
        //         color.r = 0.1 + (height / 4.0);
        //         // draw_cube(vec3(x as f32, y as f32, 0.0), vec3(1.0, 1.0, height), None, color);
        //     }
        // }
        draw_mesh(&mesh);

        // TODO: draw players
        for i in 0..MAX_PLAYERS {
            if let Some(player) = &self.game.players[i] {
                Player::draw(Vec3::new(player.position.x, player.position.y, 1.0));
                // draw_rectangle(pos.x, pos.y, 20.0, 20.0, BLACK);
                // draw_cube(vec3(player.position.x, player.position.y, 1.0), Vec3::new(1.0, 1.0, 1.0), None, BLACK);
                // let pos = cam.screen_to_world(Vec2::new(player.position.x, player.position.y));
                let end = vec3(player.position.x + 0.0, player.position.y + 5.0, 1.0);
                draw_line_3d(Vec3::new(player.position.x, player.position.y, 1.0), end, ORANGE);
                draw_text(
                    &i.to_string(),
                    player.position.x,
                    player.position.y,
                    20.0,
                    RED,
                );
            }
        }



        set_default_camera();
        if let Some(client_id) = self.client_id() {
            let text = &format!("Connected. Id#{}", client_id);
            let dim = measure_text(text, None, 20, 1.0);
            draw_text(
                text,
                screen_width() - dim.width - 20.0,
                20.0,
                20.0,
                DARKGRAY,
            );
            let activity_time = self.net().stat().activity_time();
            let (tx, rx) = (activity_time.tx.map(|t| t.elapsed().as_millis().to_string()).unwrap_or("-".to_string()),
                            activity_time.rx.map(|t| t.elapsed().as_millis().to_string()).unwrap_or("-".to_string()));
            draw_text(
                &format!("{} {}", tx, rx),
                screen_width() - dim.width - 20.0,
                50.0,
                20.0,
                DARKGRAY,
            );
        }
        draw_text(
            &self.net().process_queue_len().to_string(),
            20.0,
            20.0,
            20.0,
            DARKGRAY,
        );
        draw_text(
            &format!("{:.0}", self.fps_calc.average()),
            40.0,
            20.0,
            20.0,
            DARKGRAY,
        );
    }
}