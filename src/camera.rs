use glam::Vec3;

/// Distance the orthographic ray origin sits behind the image-plane centre (`position`).
pub const ORTHO_BACK: f32 = 150.0;

#[derive(Clone)]
pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,   // in radians
    pub pitch: f32, // in radians
    pub fov_y: f32, // in radians
    pub speed: f32,
    pub sensitivity: f32,
    /// Orthographic half-height in world units; 0 = perspective. `position` is then the
    /// centre of the image plane and the rays travel along `forward`.
    pub ortho_half_height: f32,
    /// (forward, right, up) used instead of yaw/pitch for the axis-aligned ortho views.
    pub fixed_basis: Option<(Vec3, Vec3, Vec3)>,
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
            ortho_half_height: 0.0,
            fixed_basis: None,
        }
    }

    /// Axis-aligned orthographic camera looking along `forward` with screen axes `right` / `up`.
    pub fn ortho(center: Vec3, forward: Vec3, right: Vec3, up: Vec3, half_height: f32) -> Self {
        let mut c = Camera::new(center, 0.0, 0.0);
        c.fixed_basis = Some((forward, right, up));
        c.ortho_half_height = half_height;
        c
    }

    pub fn is_ortho(&self) -> bool {
        self.ortho_half_height > 0.0
    }

    /// Zoom an orthographic view (factor < 1 zooms in)
    pub fn zoom_ortho(&mut self, factor: f32) {
        self.ortho_half_height = (self.ortho_half_height * factor).clamp(0.5, 2000.0);
    }

    pub fn forward(&self) -> Vec3 {
        if let Some((f, _, _)) = self.fixed_basis {
            return f;
        }
        Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize()
    }

    pub fn right(&self) -> Vec3 {
        if let Some((_, r, _)) = self.fixed_basis {
            return r;
        }
        self.forward().cross(Vec3::Y).normalize()
    }

    pub fn up(&self) -> Vec3 {
        if let Some((_, _, u)) = self.fixed_basis {
            return u;
        }
        self.right().cross(self.forward()).normalize()
    }

    pub fn rotate(&mut self, dx: f32, dy: f32) {
        if self.is_ortho() {
            // Orthographic views cannot rotate: dragging pans the view (about 700 px per view height)
            let k = self.ortho_half_height * 2.0 / 700.0;
            self.position += -self.right() * (dx * k) + self.up() * (dy * k);
            return;
        }
        self.yaw += dx * self.sensitivity;
        self.pitch = (self.pitch - dy * self.sensitivity).clamp(-1.50, 1.50);
    }

    pub fn move_forward(&mut self, dt: f32) {
        if self.is_ortho() {
            self.position += self.up() * (self.ortho_half_height * dt * 1.2);
            return;
        }
        self.position += self.forward() * (self.speed * dt);
    }

    pub fn move_backward(&mut self, dt: f32) {
        if self.is_ortho() {
            self.position -= self.up() * (self.ortho_half_height * dt * 1.2);
            return;
        }
        self.position -= self.forward() * (self.speed * dt);
    }

    pub fn move_right(&mut self, dt: f32) {
        if self.is_ortho() {
            self.position += self.right() * (self.ortho_half_height * dt * 1.2);
            return;
        }
        self.position += self.right() * (self.speed * dt);
    }

    pub fn move_left(&mut self, dt: f32) {
        if self.is_ortho() {
            self.position -= self.right() * (self.ortho_half_height * dt * 1.2);
            return;
        }
        self.position -= self.right() * (self.speed * dt);
    }

    pub fn move_up(&mut self, dt: f32) {
        if self.is_ortho() {
            self.zoom_ortho(1.0 - dt * 1.2); // zoom in
            return;
        }
        self.position += Vec3::Y * (self.speed * dt);
    }

    pub fn move_down(&mut self, dt: f32) {
        if self.is_ortho() {
            self.zoom_ortho(1.0 + dt * 1.2); // zoom out
            return;
        }
        self.position -= Vec3::Y * (self.speed * dt);
    }

    /// Convert a 2D screen coordinate (pixels) into a 3D ray (origin, direction)
    pub fn screen_to_ray(&self, screen_x: f32, screen_y: f32, width: f32, height: f32) -> (Vec3, Vec3) {
        let u = (screen_x / width.max(1.0)) * 2.0 - 1.0;
        let v = 1.0 - (screen_y / height.max(1.0)) * 2.0;
        let aspect = width / height.max(1.0);
        if self.is_ortho() {
            let hh = self.ortho_half_height;
            let origin = self.position + self.right() * (u * hh * aspect) + self.up() * (v * hh) - self.forward() * ORTHO_BACK;
            return (origin, self.forward());
        }
        let tan_half_fov = (self.fov_y * 0.5).tan();
        let ray_dir = (self.forward() + self.right() * (u * tan_half_fov * aspect) + self.up() * (v * tan_half_fov)).normalize();
        (self.position, ray_dir)
    }

    /// Project a 3D world position onto 2D screen viewport coordinates (for Gizmo & Selection bounds)
    pub fn project_to_screen(&self, world_point: Vec3, width: f32, height: f32) -> Option<egui::Pos2> {
        let v = world_point - self.position;
        let fwd = self.forward();
        let right = self.right();
        let up = self.up();

        if self.is_ortho() {
            let hh = self.ortho_half_height;
            let aspect = width / height.max(1.0);
            let u_ndc = v.dot(right) / (hh * aspect);
            let v_ndc = v.dot(up) / hh;
            if u_ndc < -2.0 || u_ndc > 2.0 || v_ndc < -2.0 || v_ndc > 2.0 {
                return None;
            }
            return Some(egui::pos2((u_ndc + 1.0) * 0.5 * width, (1.0 - v_ndc) * 0.5 * height));
        }

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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ViewKind {
    /// The render camera (what F9 renders)
    Camera,
    /// A second free camera for looking around without touching the render camera
    Director,
    Top,
    Front,
    Side,
}

impl ViewKind {
    pub fn is_ortho(self) -> bool {
        matches!(self, ViewKind::Top | ViewKind::Front | ViewKind::Side)
    }
}

/// Render camera + director camera + three orthographic cameras, and which one the viewport shows.
pub struct CameraRig {
    pub render: Camera,
    pub director: Camera,
    pub top: Camera,
    pub front: Camera,
    pub side: Camera,
    pub active: ViewKind,
    director_init: bool,
}

impl CameraRig {
    pub fn new(render: Camera) -> Self {
        let ortho = |f: Vec3, r: Vec3, u: Vec3| Camera::ortho(Vec3::new(0.0, 2.0, -6.0), f, r, u, 20.0);
        Self {
            director: render.clone(),
            render,
            top: ortho(-Vec3::Y, Vec3::X, -Vec3::Z),
            front: ortho(-Vec3::Z, Vec3::X, Vec3::Y),
            side: ortho(-Vec3::X, -Vec3::Z, Vec3::Y),
            active: ViewKind::Camera,
            director_init: false,
        }
    }

    pub fn active(&self) -> &Camera {
        self.get(self.active)
    }

    pub fn active_mut(&mut self) -> &mut Camera {
        match self.active {
            ViewKind::Camera => &mut self.render,
            ViewKind::Director => &mut self.director,
            ViewKind::Top => &mut self.top,
            ViewKind::Front => &mut self.front,
            ViewKind::Side => &mut self.side,
        }
    }

    pub fn get(&self, kind: ViewKind) -> &Camera {
        match kind {
            ViewKind::Camera => &self.render,
            ViewKind::Director => &self.director,
            ViewKind::Top => &self.top,
            ViewKind::Front => &self.front,
            ViewKind::Side => &self.side,
        }
    }

    /// Switch the viewport to `kind`. Ortho views are re-fitted to the scene bounds (`bounds` =
    /// min/max corner of all objects, if any); the director camera starts as a copy of the render camera.
    pub fn set_view(&mut self, kind: ViewKind, bounds: Option<(Vec3, Vec3)>, aspect: f32) {
        self.active = kind;
        if kind == ViewKind::Director && !self.director_init {
            self.director = self.render.clone();
            self.director_init = true;
        }
        if kind.is_ortho() {
            let (lo, hi) = bounds.unwrap_or((Vec3::new(-10.0, 0.0, -16.0), Vec3::new(10.0, 6.0, 4.0)));
            let center = (lo + hi) * 0.5;
            let ext = (hi - lo) * 0.5;
            let cam = self.active_mut();
            let (r, u) = (cam.right().abs(), cam.up().abs());
            let half_w = ext.dot(r);
            let half_h = ext.dot(u);
            cam.position = center;
            cam.ortho_half_height = ((half_h).max(half_w / aspect.max(0.1)) * 1.15 + 2.0).max(20.0);
        }
    }
}
