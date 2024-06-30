use crate::primitiverenderer::{Color, DrawCall, PrimitiveType};

use super::{gl, shader};
use eframe::glow;

pub struct PrimitiveRendererTexture {
    program: shader::Program,
    vertex_array: gl::VertexArray,
    vertex_buffer: gl::VertexBuffer,

    proj_model_view: nalgebra::Matrix4<f32>,
    vertices: Vec<f32>,
    max_vertices: usize,
    vertex_count: usize,
    index: usize,
    active_drawcall: Option<DrawCall>,
    draw_calls: Vec<DrawCall>,
}

/* /// Test for using a "RenderGuard" to make sure state of the renderer is correctly managed
pub struct RenderGuard<'a> {
    pr: &'a mut PrimitiveRenderer,
    pt: PrimitiveType,
    start_index: usize,
}

impl RenderGuard<'_> {
    pub fn end(self) {
        // the end of this method will drop self

        // self.pr.draw_calls.push(DrawCall {
        //     pt: self.pt,
        //     start_index: self.start_index,
        //     vertex_count: self.pr.vertex_count - self.start_index,
        // });

        // // TODO: remove
        // self.pr.active_drawcall = None;
    }
}

impl Vertex3C for RenderGuard<'_> {
    fn xyzc(&mut self, x: f32, y: f32, z: f32, color: Color) {
        self.pr.xyzc(x, y, z, color);
    }
}

impl Drop for RenderGuard<'_> {
    fn drop(&mut self) {
        self.pr.draw_calls.push(DrawCall {
            pt: self.pt,
            start_index: self.start_index,
            vertex_count: self.pr.vertex_count - self.start_index,
        });

        // TODO: remove
        self.pr.active_drawcall = None;
    }
}
 */

pub trait Vertex3C {
    /// Adds a vertex at a 3D position with a specific color
    fn xyzc(&mut self, x: f32, y: f32, z: f32, color: Color);

    #[inline]
    fn xyz(&mut self, x: f32, y: f32, z: f32) {
        self.xyzc(x, y, z, Color::BLACK);
    }

    #[inline]
    fn v3(&mut self, v: nalgebra::Vector3<f32>) {
        self.v3c(v, Color::BLACK);
    }
    #[inline]
    fn v3c(&mut self, v: nalgebra::Vector3<f32>, color: Color) {
        self.xyzc(v.x, v.y, v.z, color);
    }
}

pub trait Vertex2C {
    /// Adds a vertex at a 2D position with a specific color
    fn xyc(&mut self, x: f32, y: f32, color: Color);

    #[inline]
    fn xy(&mut self, x: f32, y: f32) {
        self.xyc(x, y, Color::BLACK);
    }

    #[inline]
    fn v2(&mut self, v: nalgebra::Vector2<f32>) {
        self.v2c(v, Color::BLACK);
    }
    #[inline]
    fn v2c(&mut self, v: nalgebra::Vector2<f32>, color: Color) {
        self.xyc(v.x, v.y, color);
    }
}

/// Automatically implement Vertex2C for any Vertex3C by setting z=0.0
impl<T: Vertex3C> Vertex2C for T {
    fn xyc(&mut self, x: f32, y: f32, color: Color) {
        self.xyzc(x, y, 0.0, color);
    }
}

pub struct RenderTexture {
    id: <eframe::glow::Context as glow::HasContext>::Texture,
}

impl PrimitiveRendererTexture {
    pub fn new(gl: &glow::Context, max_vertices: u32) -> Self {
        //load our shader
        let shader = shader::Program::new(
            gl,
            r#"
            layout(location = 0) in vec4 position;
            layout(location = 1) in vec4 color;
            layout(location = 2) in vec2 aTexCoord;
            
            uniform mat4 u_projModelView;
            
            out vec4 v_Color;
            out vec2 TexCoord;

            void main(){
                // output the final vertex position
                gl_Position = u_projModelView * position;
                    
                v_Color = vec4(color.xyz, 1.0);
                TexCoord = aTexCoord;
            }
        "#,
            r#"
            precision mediump float;
            layout(location = 0) out vec4 color;
    
            in vec4 v_Color;
            in vec2 TexCoord;

            uniform sampler2D ourTexture;

            void main(){
                color = texture(ourTexture, TexCoord) * v_Color;
                // color = v_Color;
            }
            "#,
        );

        shader.bind(gl);

        // create the layout description for the program above
        let mut layout = gl::VertexBufferLayout::new();
        layout.push(gl::GLType::Float, 3);
        layout.push(gl::GLType::UnsignedByte, 4);
        layout.push(gl::GLType::Float, 2);
        let layout = layout;

        let mut vb = gl::VertexBuffer::new(gl);

        // allocate storage for our vertices (3 position + 1 color + 2 texture coord) floats
        let vertices = vec![0f32; max_vertices as usize * (4 + 2)];

        // create vertex array and combine our vertex buffer with the layout
        let mut va = gl::VertexArray::new(gl);
        va.add_buffer(gl, &mut vb, &layout);

        Self {
            program: shader,
            vertex_array: va,
            vertex_buffer: vb,
            vertices,
            max_vertices: max_vertices as usize,
            proj_model_view: nalgebra::Matrix4::identity(),
            vertex_count: 0,
            index: 0,
            active_drawcall: None,
            draw_calls: Vec::new(),
        }
    }

    pub fn set_mvp(&mut self, mvp: nalgebra::Matrix4<f32>) {
        self.proj_model_view = mvp;
    }

    pub fn begin(&mut self, primitive_type: PrimitiveType) {
        assert!(
            self.active_drawcall.is_none(),
            "begin cannot be called twice in a row"
        );

        self.active_drawcall = Some(DrawCall {
            pt: primitive_type,
            start_index: self.vertex_count,
            vertex_count: 0,
        });
    }

    pub fn end(&mut self) {
        // mark the current position in the buffer
        if let Some(mut dc) = self.active_drawcall {
            dc.vertex_count = self.vertex_count - dc.start_index;
            self.draw_calls.push(dc);
        } else {
            panic!("end() cannot be called before a call to begin() was made");
        }

        self.active_drawcall = None;
    }

    pub fn create_texture(
        &self,
        gl: &glow::Context,
        image_data: &[u8],
        width: u32,
        height: u32,
    ) -> RenderTexture {
        use glow::HasContext as _;

        unsafe {
            let texture_id = gl.create_texture().expect("cannot create texture");
            gl.bind_texture(glow::TEXTURE_2D, Some(texture_id));
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::REPEAT as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::REPEAT as i32);
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );

            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                width as i32,
                height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(image_data),
            );
            eframe::egui_glow::check_for_gl_error!(&gl, "tex_image_2d");

            RenderTexture { id: texture_id }
        }
    }

    // TODO: add function for ensuring space for X more vertices. That could actually take in the GL context and perform a `draw` if necessary...

    pub fn flush(&mut self, gl: &glow::Context, texture: &RenderTexture) {
        use glow::HasContext as _;

        assert!(
            self.active_drawcall.is_none(),
            "end() must be called before draw()"
        );

        // println!(
        //     "Flushing {} vertices in {} draw calls => {:.2} vertices / call. Cap = {} ~= {} MB",
        //     self.vertex_count,
        //     self.draw_calls.len(),
        //     self.vertex_count as f32 / self.draw_calls.len() as f32,
        //     self.vertices.capacity(),
        //     (self.vertices.capacity() * std::mem::size_of::<f32>()) / 1024 / 1024
        // );

        // use the shader
        self.program.bind(gl);
        self.program
            .set_uniform_matrix_4_f32(gl, "u_projModelView", self.proj_model_view);

        // bind the texture
        unsafe {
            // gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(texture.id));
        }

        // upload all our data
        self.vertex_buffer.bind(gl);
        self.vertex_buffer
            .set_vertices(gl, &self.vertices[..self.index]);

        // do the actual drawing using multiple draw calls
        self.vertex_array.bind(gl);

        // TODO: go through and "optimize" the drawcalls if possible, i.e. by combining "adjacent" calls with the same primitive type

        for dc in self.draw_calls.iter() {
            // !("Drawing {} vertices", dc.vertex_count);
            unsafe {
                gl.draw_arrays(dc.pt as u32, dc.start_index as i32, dc.vertex_count as i32);
            }
        }

        // reset state
        self.vertex_count = 0;
        self.index = 0;
        self.draw_calls.clear();
    }

    pub fn destroy(&self, gl: &glow::Context) {
        self.vertex_array.destroy(gl);
        self.vertex_buffer.destroy(gl);
        self.program.destroy(gl);
    }
}

impl PrimitiveRendererTexture {
    pub fn xyzc(&mut self, x: f32, y: f32, z: f32, color: Color, texture_x: f32, texture_y: f32) {
        assert!(
            self.active_drawcall.is_some(),
            "must call begin() before vertex"
        );

        // if the buffer is full, do a "flush"
        if self.vertex_count >= self.max_vertices - 1 {
            panic!("no more space for vertices");
        }

        // SAFETY: we keep track and make sure we have enough space using index and vertex_count variables
        #[allow(clippy::identity_op)]
        unsafe {
            *self.vertices.get_unchecked_mut(self.index + 0) = x;
            *self.vertices.get_unchecked_mut(self.index + 1) = y;
            *self.vertices.get_unchecked_mut(self.index + 2) = z;
            *self.vertices.get_unchecked_mut(self.index + 3) = color.bits;
            *self.vertices.get_unchecked_mut(self.index + 4) = texture_x;
            *self.vertices.get_unchecked_mut(self.index + 5) = texture_y;
        }

        self.index += 4+2; // 3 position + 1 u32 for color + 2 for texture coord
        self.vertex_count += 1;
    }
}
