#![feature(proc_macro_hygiene)]

extern crate natord;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate warp;
extern crate clap;
extern crate maud;
#[macro_use]
extern crate serde_derive;
extern crate percent_encoding;
extern crate rand;
extern crate serde;
extern crate url;

use clap::{App, Arg};
use maud::{html, Markup, PreEscaped, DOCTYPE};
use percent_encoding::percent_decode;
use rand::Rng;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::{Mutex, RwLock};
use url::form_urlencoded;
use warp::Filter;

lazy_static! {
  static ref random_order: Mutex<String> = Mutex::new("".to_string());
  static ref input: RwLock<Input> = RwLock::new(Input {
    images_path: "".to_string(),
    images: vec![],
    save_path: Some("".to_string())
  });
}

struct Input {
  images_path: String,
  images: Vec<String>,
  save_path: Option<String>,
}

fn reshuffle(pages: usize) {
  let mut order = random_order.lock().unwrap();
  let mut shuffled_order: Vec<usize> = (0..pages).collect();
  rand::thread_rng().shuffle(&mut shuffled_order);
  *order = shuffled_order
    .into_iter()
    .map(|page| page.to_string())
    .collect::<Vec<String>>()
    .join(",");
}

fn parse_order(order: String) -> Vec<usize> {
  let ids: Result<Vec<usize>, _> = percent_decode(order.as_bytes())
    .decode_utf8_lossy()
    .split(",")
    .map(|id| id.parse())
    .collect();
  ids.unwrap_or(Vec::new())
}

fn compose_url(
  shuffle: &Option<String>,
  order: &Option<String>,
  page: usize,
  interval: &Option<u32>,
  save: bool,
) -> String {
  let mut url = form_urlencoded::Serializer::new("?".to_string());
  if let Some(interval) = interval {
    url.append_pair("interval", interval.to_string().as_ref());
  };
  if let Some(order) = order {
    url.append_pair("order", order.as_ref());
  };
  if let Some(_) = shuffle {
    url.append_pair("shuffle", "true");
  };

  if save {
    url.append_pair("save", "true");
  }

  url
    .append_pair("page", page.to_string().as_ref())
    .finish()
    .to_string()
}

fn save_image(save_path: String, image_path: String) -> Result<u64, std::io::Error> {
  std::fs::copy(image_path, save_path)
}

fn img_view_markup(q: Query) -> String {
  let &Input {
    ref images_path,
    ref images,
    ref save_path,
  } = &(*input.read().unwrap());

  let mut parsed_order = parse_order(q.order.clone().unwrap_or("".to_string()));
  let order = if let Some(_) = q.shuffle {
    reshuffle(images.len());
    let order = random_order.lock().unwrap();
    parsed_order = parse_order(order.clone());
    Some(order.clone())
  } else {
    if parsed_order.len() == 0 || parsed_order.len() != images.len() {
      None
    } else {
      q.order
    }
  };

  let mut page: usize = q.page.unwrap_or(0);
  if page >= images.len() {
    page = 0;
  }
  let prev_page = if page == 0 {
    images.len() - 1
  } else {
    page - 1
  };
  let next_page = if page == images.len() - 1 {
    0
  } else {
    page + 1
  };
  let index = match order {
    None => page,
    Some(_) => parsed_order[page],
  };

  if q.save.is_some() && save_path.is_some() {
    let result = save_image(
      Path::new(&(save_path.as_ref().unwrap()))
        .join(images[index].as_str())
        .to_string_lossy()
        .into_owned(),
      Path::new(&images_path)
        .join(images[index].as_str())
        .to_string_lossy()
        .into_owned(),
    );
    if result.is_err() {
      println!("Failed to save {}:{:?}", images[index], result);
    }
  }

  let prev_url = compose_url(&None, &order, prev_page, &q.interval, false);
  let url = String::from("/img/") + images[index].as_str();
  let next_url = compose_url(&None, &order, next_page, &q.interval, false);

  markup(
    prev_url.as_str(),
    url.as_str(),
    next_url.as_str(),
    &order,
    &q.interval,
    page,
    &images,
  )
  .into()
}

fn header(
  prev_url: &str,
  url: &str,
  next_url: &str,
  order: &Option<String>,
  interval: &Option<u32>,
  page: usize,
  images_len: usize,
) -> Markup {
  html! {
      a.header-elem href=(prev_url) { "prev" }
      a.header-elem href=(compose_url(&Some("".to_string()), order, 0, interval, false)) { "shuffle" }
      a.header-elem href=(compose_url(&None, order, page, interval, true)) { "save" }
      label.header-elem.page {
          @let view_page: u64 = page as u64 + 1;
          (view_page)"/"(images_len)
      }
      a.header-elem href=(url) { "link" }
      @if let Some(_) = interval {
          a.header-elem href=(compose_url(&None, order, page, &None, false)) { "stop" }
      } @else {
          div.dropdown-button.header-elem onclick="dropdown_toggle()" { "loop"
              div.interval-dropdown.hide {
                  a.dropdown-elem href=(compose_url(&None, order, 0, &Some(500), false)) { ".5s" }
                  a.dropdown-elem href=(compose_url(&None, order, 0, &Some(1000), false)) { "1s" }
                  a.dropdown-elem href=(compose_url(&None, order, 0, &Some(2000), false)) { "2s" }
                  a.dropdown-elem href=(compose_url(&None, order, 0, &Some(5000), false)) { "5s" }
                  a.dropdown-elem href=(compose_url(&None, order, 0, &Some(10000), false)) { "10s" }
              }
          }
      }
      a.header-elem href=(next_url) { "next" }
  }
}

fn markup(
  prev_url: &str,
  url: &str,
  next_url: &str,
  order: &Option<String>,
  interval: &Option<u32>,
  page: usize,
  images: &Vec<String>,
) -> Markup {
  html! {
      (DOCTYPE)
      html lang="en" {
          head {
              meta charset="UTF-8";
              meta name="viewport" content="width=device-width, initial-scale=1";
              title { "Image Viewer" }
              style { (include_str!("style.css")) }
          }
          body {
              div.header {
                  (header(prev_url, url, next_url, order, interval, page, images.len()))
              }
              div.img-div {
                  a.img-button href=(prev_url) { }
                  a.img-button href=(next_url) style="margin-left: 50%" { }
                  img src=(url) alt="No image";
              }
              script { (PreEscaped(include_str!("dropdown_toggle.js"))) }
              @if let Some(interval) = interval {
                  script { (PreEscaped(format!("var interval = {};", interval))) }
                  script { (PreEscaped(format!(r#"var next_page = "{}";"#, next_url))) }
                  script { (PreEscaped(include_str!("interval.js"))) }
              }
          }
      }
  }
}

#[derive(Deserialize, Debug)]
struct Query {
  interval: Option<u32>,
  page: Option<usize>,
  order: Option<String>,
  shuffle: Option<String>,
  save: Option<String>,
}

fn serve_images(address: SocketAddr) {
  let images_route = path!("img")
    .and(warp::fs::dir(input.read().unwrap().images_path.clone()))
    .with(warp::reply::with::header(
      "Cache-Control",
      "max-age=31536000",
    ));

  let index_route = warp::path::end().map(move || {
    img_view_markup(Query {
      interval: None,
      page: None,
      order: None,
      shuffle: None,
      save: None,
    })
  });

  let img_view = warp::path::end()
    .and(warp::query::query::<Query>())
    .map(move |q| img_view_markup(q));
  let routes = images_route.or(img_view).or(index_route);

  warp::serve(routes).run(address);
}

trait IsPicturer {
  fn is_picture(&self) -> bool;
}
impl IsPicturer for std::path::Path {
  fn is_picture(&self) -> bool {
    match self.extension() {
      Some(ext) => ["png", "jpeg", "jpg", "gif"]
        .into_iter()
        .any(|v| v == &ext.to_string_lossy().to_lowercase()),
      _ => false,
    }
  }
}

fn main() {
  let matches = App::new("Image server")
    .version(env!("CARGO_PKG_VERSION"))
    .arg(Arg::with_name("address").long("address").takes_value(true))
    .arg(Arg::with_name("save").long("save").takes_value(true))
    .arg(Arg::from_usage("<dir> 'image directory'"))
    .get_matches();

  let address = matches
    .value_of("address")
    .map(|a| a.parse().expect("Invalid address"))
    .unwrap_or(SocketAddr::new(
      IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
      8080,
    ));

  let images_path = matches.value_of("dir").unwrap();

  let mut images: Vec<String> = std::fs::read_dir(images_path)
    .expect("Failed to open images directory")
    .filter_map(|de| de.ok())
    .filter(|de| {
      de.metadata().is_ok() && de.metadata().unwrap().is_file() && de.path().is_picture()
    })
    .map(|de| de.file_name().into_string())
    .filter_map(|filename| filename.ok())
    .collect();
  images.sort_by(|a, b| natord::compare(a.as_str(), b.as_str()));

  let save_path = matches.value_of("save").map(|s| s.to_string());
  match save_path {
    Some(ref path) => {
      if !(std::path::Path::new(path.as_str()).exists()) {
        panic!("Invalid save path")
      }
    }
    None => (),
  }

  let mut input_mut = input.write().unwrap();
  input_mut.images_path = images_path.to_string();
  input_mut.images = images;
  input_mut.save_path = save_path;
  drop(input_mut);

  serve_images(address);
}
