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
}
