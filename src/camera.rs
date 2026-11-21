use glam::Vec3;

pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,   // in radians
    pub pitch: f32, // in radians
    pub fov_y: f32, // in radians
    pub speed: f32,
    pub sensitivity: f32,
}

impl Camera {
    pub fn new(position: Vec3, yaw_deg: f32, pitch_deg: f32) -> Self {
        Self {
            position,
            yaw: yaw_deg.to_radians(),
            pitch: pitch_deg.to_radians(),
            fov_y: 60.0_f32.to_radians(),
            speed: 8.0,
            sensitivity: 0.003,
        }
    }

    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize()
    }

    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize()
    }

    pub fn up(&self) -> Vec3 {
        self.right().cross(self.forward()).normalize()
    }

    pub fn rotate(&mut self, dx: f32, dy: f32) {
        self.yaw += dx * self.sensitivity;
        self.pitch = (self.pitch - dy * self.sensitivity).clamp(-1.50, 1.50);
    }

    pub fn move_forward(&mut self, dt: f32) {
        self.position += self.forward() * (self.speed * dt);
    }

    pub fn move_backward(&mut self, dt: f32) {
        self.position -= self.forward() * (self.speed * dt);
    }

    pub fn move_right(&mut self, dt: f32) {
        self.position += self.right() * (self.speed * dt);
    }

    pub fn move_left(&mut self, dt: f32) {
        self.position -= self.right() * (self.speed * dt);
    }

    pub fn move_up(&mut self, dt: f32) {
        self.position += Vec3::Y * (self.speed * dt);
    }

    pub fn move_down(&mut self, dt: f32) {
        self.position -= Vec3::Y * (self.speed * dt);
    }

    /// Convert a 2D screen coordinate (pixels) into a 3D ray (origin, direction)
    pub fn screen_to_ray(&self, screen_x: f32, screen_y: f32, width: f32, height: f32) -> (Vec3, Vec3) {
        let u = (screen_x / width.max(1.0)) * 2.0 - 1.0;
        let v = 1.0 - (screen_y / height.max(1.0)) * 2.0;
        let tan_half_fov = (self.fov_y * 0.5).tan();
        let aspect = width / height.max(1.0);
        let ray_dir = (self.forward() + self.right() * (u * tan_half_fov * aspect) + self.up() * (v * tan_half_fov)).normalize();
        (self.position, ray_dir)
    }

    /// Project a 3D world position onto 2D screen viewport coordinates (for Gizmo & Selection bounds)
    pub fn project_to_screen(&self, world_point: Vec3, width: f32, height: f32) -> Option<egui::Pos2> {
        let v = world_point - self.position;
        let fwd = self.forward();
        let right = self.right();
        let up = self.up();

        let z_view = v.dot(fwd);
        if z_view <= 0.1 {
            return None; // Point is behind or too close to the camera
        }

        let x_view = v.dot(right);
        let y_view = v.dot(up);

        let tan_half_fov = (self.fov_y * 0.5).tan();
        let aspect = width / height.max(1.0);

        let u_ndc = x_view / (z_view * tan_half_fov * aspect);
        let v_ndc = y_view / (z_view * tan_half_fov);

        if u_ndc < -2.0 || u_ndc > 2.0 || v_ndc < -2.0 || v_ndc > 2.0 {
            return None; // Outside visible frustum
        }

        let screen_x = (u_ndc + 1.0) * 0.5 * width;
        let screen_y = (1.0 - v_ndc) * 0.5 * height;

        Some(egui::pos2(screen_x, screen_y))
    }
}
