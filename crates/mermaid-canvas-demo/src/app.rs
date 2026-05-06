//! 应用窗口
//!
//! 使用 winit 创建窗口，softbuffer 管理表面，将 Pixmap 渲染到屏幕。
//! 自动处理 DPI 缩放。

use std::num::NonZeroU32;
use std::sync::Arc;

/// 演示应用
pub struct DemoApp {
    title: String,
}

impl DemoApp {
    /// 创建新的演示应用
    pub fn new(title: &str, _width: u32, _height: u32) -> Self {
        Self {
            title: title.to_string(),
        }
    }

    /// 运行应用，显示渲染结果
    ///
    /// 自动根据系统 DPI 缩放调整 Surface 尺寸，
    /// 确保 Pixmap 内容填满整个窗口。
    pub fn run(self, pixmap: tiny_skia::Pixmap) -> Result<(), Box<dyn std::error::Error>> {
        use winit::application::ApplicationHandler;
        use winit::event::WindowEvent;
        use winit::event_loop::{ActiveEventLoop, EventLoop};
        use winit::window::{Window, WindowAttributes};

        struct App {
            pixmap: tiny_skia::Pixmap,
            title: String,
            window: Option<Arc<Window>>,
        }

        impl ApplicationHandler for App {
            fn resumed(&mut self, event_loop: &ActiveEventLoop) {
                if self.window.is_some() {
                    return;
                }

                let attrs = WindowAttributes::default()
                    .with_title(&self.title)
                    .with_inner_size(winit::dpi::LogicalSize::new(
                        self.pixmap.width(),
                        self.pixmap.height(),
                    ));

                let window = Arc::new(event_loop.create_window(attrs).unwrap());
                self.window = Some(window.clone());

                // 获取 DPI 缩放因子
                let scale_factor = window.scale_factor() as u32;
                let surface_w = self.pixmap.width() * scale_factor;
                let surface_h = self.pixmap.height() * scale_factor;

                let ctx = softbuffer::Context::new(&*window)
                    .expect("Failed to create softbuffer context");
                let mut surface = softbuffer::Surface::new(&ctx, &*window)
                    .expect("Failed to create softbuffer surface");

                let _ = surface.resize(
                    NonZeroU32::new(surface_w).unwrap(),
                    NonZeroU32::new(surface_h).unwrap(),
                );

                let mut buffer = surface
                    .buffer_mut()
                    .expect("Failed to get surface buffer");

                // 按 scale_factor 放大像素：每个逻辑像素映射到 scale_factor×scale_factor 物理像素
                let pw = self.pixmap.width();
                let ph = self.pixmap.height();
                let pixels = self.pixmap.pixels();

                for sy in 0..surface_h {
                    let src_y = (sy / scale_factor) as usize;
                    if src_y >= ph as usize {
                        break;
                    }
                    for sx in 0..surface_w {
                        let src_x = (sx / scale_factor) as usize;
                        if src_x >= pw as usize {
                            break;
                        }
                        let pixel = pixels[src_y * pw as usize + src_x];
                        let idx = (sy * surface_w + sx) as usize;
                        buffer[idx] = u32::from(pixel.red()) << 16
                            | u32::from(pixel.green()) << 8
                            | u32::from(pixel.blue());
                    }
                }

                buffer.present().expect("Failed to present buffer");
            }

            fn window_event(
                &mut self,
                event_loop: &ActiveEventLoop,
                _window_id: winit::window::WindowId,
                event: WindowEvent,
            ) {
                match event {
                    WindowEvent::CloseRequested => {
                        event_loop.exit();
                    }
                    WindowEvent::RedrawRequested => {}
                    _ => {}
                }
            }
        }

        let event_loop = EventLoop::new()?;
        let mut app = App {
            pixmap,
            title: self.title,
            window: None,
        };

        event_loop.run_app(&mut app)?;
        Ok(())
    }
}
