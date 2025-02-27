use anyhow::{anyhow, Result};
use dbus::{
  arg::{AppendAll, Iter, IterAppend, PropMap, ReadAll, RefArg, TypeMismatchError, Variant},
  blocking::Connection,
  message::{MatchRule, SignalArgs},
};
use png::Decoder;
use std::{
  collections::HashMap,
  env::temp_dir,
  fs::{self, File},
  sync::{Arc, Mutex},
  time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Debug)]
pub struct OrgFreedesktopPortalRequestResponse {
  pub status: u32,
  pub results: PropMap,
}

impl AppendAll for OrgFreedesktopPortalRequestResponse {
  fn append(&self, i: &mut IterAppend) {
    RefArg::append(&self.status, i);
    RefArg::append(&self.results, i);
  }
}

impl ReadAll for OrgFreedesktopPortalRequestResponse {
  fn read(i: &mut Iter) -> Result<Self, TypeMismatchError> {
    Ok(OrgFreedesktopPortalRequestResponse {
      status: i.read()?,
      results: i.read()?,
    })
  }
}

impl SignalArgs for OrgFreedesktopPortalRequestResponse {
  const NAME: &'static str = "Response";
  const INTERFACE: &'static str = "org.freedesktop.portal.Request";
}

fn org_gnome_shell_screenshot(
  conn: &Connection,
  x: i32,
  y: i32,
  width: i32,
  height: i32,
) -> Result<Vec<u8>> {
  let proxy = conn.with_proxy(
    "org.gnome.Shell.Screenshot",
    "/org/gnome/Shell/Screenshot",
    Duration::from_secs(10),
  );

  let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?;

  let dirname = temp_dir().join("screenshot");

  fs::create_dir_all(&dirname)?;

  let mut path = dirname.join(timestamp.as_micros().to_string());
  path.set_extension("png");

  let filename = path.to_string_lossy().to_string();

  proxy.method_call(
    "org.gnome.Shell.Screenshot",
    "ScreenshotArea",
    (x, y, width, height, false, &filename),
  )?;

  let buffer = fs::read(&filename)?;
  fs::remove_file(&filename)?;

  Ok(buffer)
}

fn org_freedesktop_portal_screenshot(
  conn: &Connection,
  x: i32,
  y: i32,
  width: i32,
  height: i32,
) -> Result<Vec<u8>> {
  let status: Arc<Mutex<Option<u32>>> = Arc::new(Mutex::new(None));
  let status_res = status.clone();
  let path: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
  let path_res = path.clone();

  let match_rule = MatchRule::new_signal("org.freedesktop.portal.Request", "Response");
  conn.add_match(
    match_rule,
    move |response: OrgFreedesktopPortalRequestResponse, _conn, _msg| {
      if let Ok(mut status) = status.lock() {
        *status = Some(response.status);
      }

      let uri = response.results.get("uri").and_then(|str| str.as_str());
      if let (Some(uri_str), Ok(mut path)) = (uri, path.lock()) {
        *path = uri_str[7..].to_string();
      }

      true
    },
  )?;

  let proxy = conn.with_proxy(
    "org.freedesktop.portal.Desktop",
    "/org/freedesktop/portal/desktop",
    Duration::from_millis(10000),
  );

  let mut options: PropMap = HashMap::new();
  options.insert(
    String::from("handle_token"),
    Variant(Box::new(String::from("1234"))),
  );
  options.insert(String::from("modal"), Variant(Box::new(true)));
  options.insert(String::from("interactive"), Variant(Box::new(false)));

  proxy.method_call(
    "org.freedesktop.portal.Screenshot",
    "Screenshot",
    ("", options),
  )?;

  // wait 60 seconds for user interaction
  for _ in 0..60 {
    let result = conn.process(Duration::from_millis(1000))?;
    let status = status_res
      .lock()
      .map_err(|_| anyhow!("Get status lock failed"))?;

    if result && status.is_some() {
      break;
    }
  }

  let status = status_res
    .lock()
    .map_err(|_| anyhow!("Get status lock failed"))?;
  let status = *status;

  let path = path_res
    .lock()
    .map_err(|_| anyhow!("Get path lock failed"))?;
  let path = &*path;

  if status.ne(&Some(0)) || path.is_empty() {
    if !path.is_empty() {
      fs::remove_file(path)?;
    }
    return Err(anyhow!("Screenshot failed or canceled",));
  }
  let iter = percent_encoding::percent_decode(path.as_bytes());
  let decoded_path = iter.decode_utf8()?;
  let decoder = Decoder::new(File::open(decoded_path.to_string())?);

  let mut reader = decoder.read_info()?;
  // Allocate the output buffer.
  let mut buf = vec![0u8; reader.output_buffer_size()];
  // Read the next frame. An APNG might contain multiple frames.
  let info = reader.next_frame(&mut buf)?;
  // Grab the bytes of the image.
  let bytes = &buf[..info.buffer_size()];

  fs::remove_file(decoded_path.to_string())?;

  let mut rgba = vec![0u8; (width * height * 4) as usize];
  // 图片裁剪
  for r in y..(y + height) {
    for c in x..(x + width) {
      let index = (((r - y) * width + (c - x)) * 4) as usize;
      let i = ((r * info.width as i32 + c) * 4) as usize;
      // 防止获取到的图片尺寸小于指定大小而 panic
      rgba[index] = bytes.get(i).copied().unwrap_or(0);
      rgba[index + 1] = bytes.get(i + 1).copied().unwrap_or(0);
      rgba[index + 2] = bytes.get(i + 2).copied().unwrap_or(0);
      rgba[index + 3] = bytes.get(i + 3).copied().unwrap_or(0);
    }
  }

  Ok(rgba)
}

// TODO: 失败后尝试删除文件
pub fn wayland_screenshot(x: i32, y: i32, width: i32, height: i32) -> Result<Vec<u8>> {
  let conn = Connection::new_session()?;

  org_gnome_shell_screenshot(&conn, x, y, width, height)
    .or_else(|_| org_freedesktop_portal_screenshot(&conn, x, y, width, height))
}
