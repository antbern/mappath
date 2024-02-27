pub struct Camera {
    offset: (f64, f64),
    scale: f64,
}

/// A camera that can be used to pan and zoom the view
impl Camera {
    pub fn new(initial_scale: f64) -> Camera {
        Camera {
            offset: (0.0, 0.0),
            scale: initial_scale,
        }
    }

    /// Converts a pixel position into a world position
    pub fn pixel_to_world(&self, x: i32, y: i32) -> (f64, f64) {
        let (x, y) = (x as f64, y as f64);
        let (x, y) = (x / self.scale, y / self.scale);
        (x - self.offset.0, y - self.offset.1)
    }

    /// Pans the camera by the given amount in pixels
    pub fn pan_pixels(&mut self, dx: i32, dy: i32) {
        self.offset.0 += dx as f64 / self.scale;
        self.offset.1 += dy as f64 / self.scale;
    }

    pub fn zoom_at(&mut self, x: i32, y: i32, factor: f64) {
        let (x, y) = (x as f64, y as f64);

        // x and y need to be the location on the canvas, not in the world
        let (x, y) = (x / self.scale, y / self.scale);

        self.scale *= factor;
        self.offset.0 -= x * (1.0 - 1.0 / factor);
        self.offset.1 -= y * (1.0 - 1.0 / factor);
    }

    pub fn scale(&self) -> f64 {
        self.scale
    }

    pub fn offset(&self) -> (f64, f64) {
        self.offset
    }
}
