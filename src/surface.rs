//! Utility functions for creating dpi aware wayland surfaces.
use crate::env::Environment;
use crate::output::OutputMgr;
use std::sync::Mutex;
use wayland_client::protocol::{wl_output, wl_surface};

pub(crate) struct SurfaceUserData {
    dpi_factor: i32,
    outputs: Vec<wl_output::WlOutput>,
    output_manager: OutputMgr,
    dpi_change_cb: Box<dyn FnMut(i32, wl_surface::WlSurface) + Send + 'static>,
}

impl SurfaceUserData {
    fn new(
        output_manager: OutputMgr,
        dpi_change_cb: Box<dyn FnMut(i32, wl_surface::WlSurface) + Send + 'static>,
    ) -> Self {
        SurfaceUserData {
            dpi_factor: 1,
            outputs: Vec::new(),
            output_manager,
            dpi_change_cb,
        }
    }

    pub(crate) fn enter(&mut self, output: wl_output::WlOutput, surface: wl_surface::WlSurface) {
        self.outputs.push(output);
        self.compute_dpi_factor(surface);
    }

    pub(crate) fn leave(&mut self, output: &wl_output::WlOutput, surface: wl_surface::WlSurface) {
        self.outputs
            .retain(|output2| !output.as_ref().equals(output2.as_ref()));
        self.compute_dpi_factor(surface);
    }

    fn compute_dpi_factor(&mut self, surface: wl_surface::WlSurface) {
        let mut scale_factor = 1;
        for output in &self.outputs {
            if let Some(scale_factor2) = self
                .output_manager
                .with_info(&output, |_id, info| info.scale_factor)
            {
                scale_factor = ::std::cmp::max(scale_factor, scale_factor2);
            }
        }
        if self.dpi_factor != scale_factor {
            self.dpi_factor = scale_factor;
            (self.dpi_change_cb)(scale_factor, surface.clone());
        }
    }
}

/// Creates a WlSurface from an Environment.
///
/// Computes the dpi factor by taking the maximum dpi value of all the outputs
/// the surface is displayed on. When the dpi value is updated the caller is
/// notified through the dpi_change closure.
pub(crate) fn create_surface<F>(
    environment: &Environment,
    dpi_change: Box<F>,
) -> wl_surface::WlSurface
where
    F: FnMut(i32, wl_surface::WlSurface) + Send + 'static,
{
    let surface = environment.compositor.create_surface();
    surface.assign_mono(move |surface, event| {
        let mut user_data = surface
            .as_ref()
            .user_data()
            .get::<Mutex<SurfaceUserData>>()
            .unwrap()
            .lock()
            .unwrap();
        match event {
            wl_surface::Event::Enter { output } => {
                user_data.enter(output, (*surface).clone().detach());
            }
            wl_surface::Event::Leave { output } => {
                user_data.leave(&output, (*surface).clone().detach());
            }
            _ => unreachable!(),
        };
    });
    surface.as_ref().user_data().set_threadsafe(|| {
        Mutex::new(SurfaceUserData::new(
            environment.outputs.clone(),
            dpi_change,
        ))
    });
    (*surface).clone().detach()
}

/// Returns the current dpi factor of a surface.
pub fn get_dpi_factor(surface: &wl_surface::WlSurface) -> i32 {
    surface
        .as_ref()
        .user_data()
        .get::<Mutex<SurfaceUserData>>()
        .expect("Surface was not created with create_surface.")
        .lock()
        .unwrap()
        .dpi_factor
}

/// Returns a list of outputs the surface is displayed on.
pub fn get_outputs(surface: &wl_surface::WlSurface) -> Vec<wl_output::WlOutput> {
    surface
        .as_ref()
        .user_data()
        .get::<Mutex<SurfaceUserData>>()
        .expect("Surface was not created with create_surface.")
        .lock()
        .unwrap()
        .outputs
        .clone()
}
