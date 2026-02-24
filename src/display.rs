use anyhow::{Result, anyhow};
use core_graphics::display::{
    CGDirectDisplayID, CGDisplayBounds, CGDisplayPixelsHigh, CGDisplayPixelsWide,
    CGGetActiveDisplayList,
};

#[derive(Debug, Clone)]
pub struct DisplayInfo {
    pub display_id: u32,
    pub logical_width: u32,
    pub logical_height: u32,
    pub origin_x: i32,
    pub origin_y: i32,
    pub scale_factor: f64,
}

#[derive(Debug, Clone)]
pub struct StitchedLayout {
    pub displays: Vec<DisplayInfo>,
    pub stitched_width: u32,
    pub stitched_height: u32,
    pub offsets: Vec<(u32, u32)>,
}

pub fn get_active_displays() -> Result<Vec<DisplayInfo>> {
    const MAX_DISPLAYS: usize = 32;

    let mut ids: [CGDirectDisplayID; MAX_DISPLAYS] = [0; MAX_DISPLAYS];
    let mut count: u32 = 0;

    let err = unsafe {
        CGGetActiveDisplayList(
            MAX_DISPLAYS as u32,
            ids.as_mut_ptr(),
            &mut count as *mut u32,
        )
    };

    if err != 0 {
        return Err(anyhow!("CGGetActiveDisplayList 失败，错误码: {}", err));
    }

    let mut displays = Vec::with_capacity(count as usize);

    for id in ids.into_iter().take(count as usize) {
        let bounds = unsafe { CGDisplayBounds(id) };

        let logical_width = bounds.size.width.round().max(1.0) as u32;
        let logical_height = bounds.size.height.round().max(1.0) as u32;

        let pixel_width = unsafe { CGDisplayPixelsWide(id) } as f64;
        let _pixel_height = unsafe { CGDisplayPixelsHigh(id) } as f64;
        let scale_factor = (pixel_width / logical_width as f64).max(1.0);

        displays.push(DisplayInfo {
            display_id: id,
            logical_width,
            logical_height,
            origin_x: bounds.origin.x.round() as i32,
            origin_y: bounds.origin.y.round() as i32,
            scale_factor,
        });
    }

    displays.sort_by_key(|d| d.display_id);
    Ok(displays)
}

pub fn get_stitched_layout() -> Result<StitchedLayout> {
    let displays = get_active_displays()?;
    if displays.is_empty() {
        return Err(anyhow!("未获取到活跃显示器信息"));
    }

    let mut stitched_width = 0u32;
    let mut stitched_height = 0u32;
    let mut offsets = Vec::with_capacity(displays.len());

    for display in &displays {
        offsets.push((stitched_width, 0));
        stitched_width = stitched_width.saturating_add(display.logical_width);
        stitched_height = stitched_height.max(display.logical_height);
    }

    Ok(StitchedLayout {
        displays,
        stitched_width,
        stitched_height,
        offsets,
    })
}

impl StitchedLayout {
    pub fn normalized_to_global(&self, norm_x: f64, norm_y: f64) -> Result<(i32, i32), String> {
        if self.displays.is_empty() {
            return Err("没有可用显示器布局".to_string());
        }
        if self.stitched_width == 0 || self.stitched_height == 0 {
            return Err("拼接布局尺寸无效".to_string());
        }

        let nx = norm_x.clamp(0.0, 999.0);
        let ny = norm_y.clamp(0.0, 999.0);

        let max_x = self.stitched_width.saturating_sub(1) as f64;
        let max_y = self.stitched_height.saturating_sub(1) as f64;

        let pixel_x = (nx / 999.0 * self.stitched_width as f64).clamp(0.0, max_x);
        let pixel_y = (ny / 999.0 * self.stitched_height as f64).clamp(0.0, max_y);

        for (i, display) in self.displays.iter().enumerate() {
            let (ox, oy) = self.offsets[i];
            let right = ox.saturating_add(display.logical_width);
            let bottom = oy.saturating_add(display.logical_height);

            if pixel_x >= ox as f64
                && pixel_x < right as f64
                && pixel_y >= oy as f64
                && pixel_y < bottom as f64
            {
                let local_x = pixel_x - ox as f64;
                let local_y = pixel_y - oy as f64;

                let global_x = display.origin_x + local_x.floor() as i32;
                let global_y = display.origin_y + local_y.floor() as i32;
                return Ok((global_x, global_y));
            }
        }

        Err(format!(
            "坐标超出所有屏幕范围: norm=({:.2}, {:.2}), pixel=({:.2}, {:.2})",
            norm_x, norm_y, pixel_x, pixel_y
        ))
    }
}
