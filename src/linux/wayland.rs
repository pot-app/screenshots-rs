use crate::{linux::wayland_screenshot::wayland_screenshot, DisplayInfo, Image};
use anyhow::Result;

pub fn wayland_capture_screen(display_info: &DisplayInfo) -> Result<Image> {
  let x = ((display_info.x as f32) * display_info.scale_factor) as i32;
  let y = ((display_info.y as f32) * display_info.scale_factor) as i32;
  let width = (display_info.width as f32) * display_info.scale_factor;
  let height = (display_info.height as f32) * display_info.scale_factor;

  let rgba = wayland_screenshot(x, y, width as i32, height as i32)?;

  Ok(Image::new(width as u32, height as u32, rgba))
}

pub fn wayland_capture_screen_area(
  display_info: &DisplayInfo,
  x: i32,
  y: i32,
  width: u32,
  height: u32,
) -> Result<Image> {
  let area_x = (((x + display_info.x) as f32) * display_info.scale_factor) as i32;
  let area_y = (((y + display_info.y) as f32) * display_info.scale_factor) as i32;
  let area_width = (width as f32) * display_info.scale_factor;
  let area_height = (height as f32) * display_info.scale_factor;

  let rgba = wayland_screenshot(area_x, area_y, area_width as i32, area_height as i32)?;

  Ok(Image::new(width as u32, height as u32, rgba))
}
