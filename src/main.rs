use notify_rust::Notification;
use std::io::BufReader;
use std::thread;
use rodio::{Decoder, OutputStream, source::Source};
use std::sync::{Arc, Mutex};
use std::{thread::sleep, time::Duration};
use image::{ImageBuffer, Rgba};
use log::{info, warn, LevelFilter};
use xcap::Window;
use windows::Win32::{Foundation::HWND, UI::Input::KeyboardAndMouse::MapVirtualKeyA};
use windows::Win32::UI::WindowsAndMessaging::{ClipCursor, FindWindowW, GetClipCursor, GetCursorPos, GetForegroundWindow, SetCursorPos, WM_LBUTTONDOWN, WM_LBUTTONUP};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_F, VK_LBUTTON};
use windows::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, PostMessageW, WM_KEYDOWN, WM_KEYUP};
use windows::Win32::Foundation::{LPARAM, RECT, WPARAM};
use windows_hotkeys::keys::{ModKey, VKey};
use windows_hotkeys::{HotkeyManager, HotkeyManagerImpl};

#[derive(Debug,Clone)]
enum InputMode {
    Keyboard,
    Mouse,
}

#[derive(Debug,Clone)]
struct ColorRow {
    color: Rgba<u8>,
    count: u32,
    //矩形范围比值：左上角(x1,y1)和右下角(x2,y2)与图片宽(width)高(height)的比值
    range_scale: (f32,f32,f32,f32),
    mode: InputMode,
    n: u32,
    last_x: u32,
    last_y: u32,
}

impl ColorRow {
    fn new(color: Rgba<u8>, count: u32, range_scale: (f32,f32,f32,f32), mode:InputMode) -> Self {
        ColorRow {
            color,
            count,
            range_scale,
            mode,
            n: 0,
            last_x: 0,
            last_y: 0,
        }
    }
}

fn capture_yuanshen_image() -> Option<ImageBuffer<Rgba<u8>, Vec<u8>>> {
    let windows = Window::all().unwrap();
    for window in windows {
        // 最小化的窗口不能截屏
        if window.is_minimized().unwrap() {
            continue;
        }
        
        let title = window.title().unwrap();
        let app_name = window.app_name().unwrap();
        if title.contains("原神") && app_name.contains("YuanShen") {
            let image = window.capture_image().unwrap();
            return Some(image);
        }else {
            continue;
        }

    }
    None
}

// 按行找色
fn find_pixel_chat(image: &ImageBuffer<Rgba<u8>, Vec<u8>>, mut target_all: Vec<ColorRow>) -> Option<ColorRow> {

    for (x, y, pixel) in image.enumerate_pixels() {
        for target in target_all.iter_mut() {
            let range_x = (image.width() as f32 / target.range_scale.0) as u32..(image.width() as f32 / target.range_scale.2) as u32;
            let range_y = (image.height() as f32 / target.range_scale.1) as u32..(image.height() as f32 / target.range_scale.3) as u32;
            //println!("{:?} , {:?} , {:?}, ({x},{y})",&target,range_x,range_y);
            if target.color == *pixel && range_x.contains(&x) && range_y.contains(&y) {
                
                if target.last_y == y && target.last_x + 1 == x {
                    target.n +=1;
                    if target.n > target.count {
                        return Some(target.clone());
                    }
                }
                target.last_y = y;
                target.last_x = x;
            }
        }
        
    }
    None
}

fn skip_chat(gi_hwnd: &HWND, mode: &InputMode) {
    unsafe {
        let _ = SetForegroundWindow(*gi_hwnd);
        match mode {
            InputMode::Keyboard => {
                let vkey = MapVirtualKeyA(VK_F.0 as u32, windows::Win32::UI::Input::KeyboardAndMouse::MAP_VIRTUAL_KEY_TYPE(0));
                // 发送按下F键消息
                let _ = PostMessageW(Some(*gi_hwnd), WM_KEYDOWN, WPARAM(VK_F.0 as usize), LPARAM((0x0001 | vkey << 16) as isize));
                // 发送释放F键消息
                let _ = PostMessageW(Some(*gi_hwnd), WM_KEYUP, WPARAM(VK_F.0 as usize), LPARAM((0x0001 | vkey << 16 | 0xC0 << 24) as isize));
            },
            InputMode::Mouse => {
                let vkey = MapVirtualKeyA(VK_LBUTTON.0 as u32, windows::Win32::UI::Input::KeyboardAndMouse::MAP_VIRTUAL_KEY_TYPE(0));
                // 发送按下鼠标消息
                let _ = PostMessageW(Some(*gi_hwnd), WM_LBUTTONDOWN, WPARAM(VK_LBUTTON.0 as usize), LPARAM((0x0001 | vkey << 16) as isize));
                // 发送释放鼠标消息
                let _ = PostMessageW(Some(*gi_hwnd), WM_LBUTTONUP, WPARAM(VK_LBUTTON.0 as usize), LPARAM((0x0001 | vkey << 16 | 0xC0 << 24) as isize));

            }
        }
        
        

        
    }
    //info!("跳过对话");
}

fn send_notify(title: &str, body: &str) {
    let _ = Notification::new()
    .summary(title)
    .body(body)
    .show();
}

fn play_audio(audio: &'static [u8]){
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let file = BufReader::new(std::io::Cursor::new(audio));
    let source = Decoder::new(file).unwrap();
    let _ = stream_handle.play_raw(source.convert_samples());
    sleep(Duration::from_secs(3));
}



fn app_run(is_start: Arc<Mutex<bool>>) -> Result<(), Box<dyn std::error::Error>>{
    env_logger::builder()
        .filter_module("genshin_auto", LevelFilter::Info)
        .init();
    println!(
        "{}",
        r#"
   ____                        _       _                _              _           
  / ___|   ___   _ __    ___  | |__   (_)  _ __        / \     _   _  | |_    ___  
 | |  _   / _ \ | '_ \  / __| | '_ \  | | | '_ \      / _ \   | | | | | __|  / _ \ 
 | |_| | |  __/ | | | | \__ \ | | | | | | | | | |    / ___ \  | |_| | | |_  | (_) |
  \____|  \___| |_| |_| |___/ |_| |_| |_| |_| |_|   /_/   \_\  \__,_|  \__|  \___/ 

                                Create by TickTock

                        一个可实现原神后台自动对话的工具

                                1.支持热键开关工具
                                2.支持对话完成后提示

            使用说明：  以管理员模式打开后自动生效，Alt + P 暂停/启动自动对话
            注意事项：  请保证游戏为无边框/窗口模式，且不能最小化
        "#
    );
    info!("开始运行");
    let mut switch_exist = false;
    let mut point = windows::Win32::Foundation::POINT { x: 0, y: 0 };
    loop {
        let color_row_all = vec![
            ColorRow::new(
                Rgba([0x3B, 0x43, 0x54, 255]),
                5,
                (2560.0/240.0,1440.0/19.0,2560.0/440.0,1440.0/89.0),
                InputMode::Keyboard,
            ),
            ColorRow::new(
                Rgba([0x4A, 0x52, 0x65, 255]),
                50,
                (2560.0/1156.0,1440.0/883.0,2560.0/1404.0,1440.0/925.0),
                InputMode::Mouse,
            ),
            ColorRow::new(
                Rgba([0x00, 0x00, 0x00, 255]),
                100,
                (2560.0/1180.0,1440.0/1200.0,2560.0/1400.0,1440.0/1267.0),
                InputMode::Mouse,
            ),
        ];
        sleep(Duration::from_millis(1));
        if let Ok(s) = is_start.lock() {
            //info!("app_run:{}",*p);
            if *s{
                if let Ok(gi_hwnd) = unsafe {FindWindowW(windows::core::w!("UnityWndClass"), windows::core::w!("原神"))} {
                    if switch_exist {
                        info!("找到原神窗口");
                        switch_exist = false;
                    }
                    sleep(Duration::from_millis(100));
                    let image = capture_yuanshen_image();
                    
                    if image.is_some() {
                        //info!("截屏成功");
                        let image = image.unwrap();
                        match find_pixel_chat(&image,color_row_all) {
                            Some(color_row) => {
                                //info!("找到目标像素: ({}, {})", target.last_x, target.last_y);
                                let mode = color_row.mode;
                                skip_chat(&gi_hwnd, &mode);
                            }
                            None => {
                                unsafe {
                                    if GetForegroundWindow() != gi_hwnd {
                                        let mut rect = RECT::default();
                                        let _ = GetClipCursor(&mut rect);
                                        //由于原神对话结束后会锁定鼠标，所以需要先解锁鼠标并移动鼠标到上一次的位置
                                        if rect.left==rect.right && rect.top==rect.bottom && rect!=RECT::default() {
                                            let _ = ClipCursor(None);
                                            // 移动鼠标到 point 上一次鼠标位置
                                            let _ = SetCursorPos(point.x, point.y);
                                            info!("对话已结束");
                                            send_notify("Genshin Auto", "对话已结束");
                                        }else {
                                            let _ = GetCursorPos(&mut point);
                                        }
                                    }
                                }
                            }

                        }
                    }
                }else {
                    if !switch_exist {
                        info!("未找到原神窗口");
                        switch_exist = true;
                    }
                }
                
            }
        }

        
        
    }
    
    
}


fn main() -> Result<(), Box<dyn std::error::Error>>{
    static TURN_ON: &'static [u8] = include_bytes!("../assets/audio/turn_on.mp3");
    static TURN_OFF: &'static [u8] = include_bytes!("../assets/audio/turn_off.mp3");
    let is_start = Arc::new(Mutex::new(true));
    let is_start_hkm = Arc::clone(&is_start);
    thread::spawn(move || {
        let mut hkm = HotkeyManager::new();
        
        let register = hkm.register(VKey::P, &[ModKey::Alt],  move|| {
            
            
            if let Ok(mut s) = is_start_hkm.lock() {
                if *s {
                    *s = false;
                    info!("已暂停");
                    thread::spawn(move || {
                        play_audio(TURN_OFF);
                    });
                } else {
                    *s = true;
                    info!("已启动");
                    thread::spawn(move || {
                        play_audio(TURN_ON);
                    });
                }
                //info!("hkm:{}",*mm);
            }
            
            
        });
        if register.is_err() {
            warn!("注册热键失败，请检查是否有其他程序占用该热键！");
        }
        hkm.event_loop();
    });
    let is_start_app = Arc::clone(&is_start);
    let task = thread::spawn(move || {
        let _ = app_run(is_start_app);
    });
    task.join().unwrap();
    //杀死task线程
    Ok(())
}


#[test]
fn test(){
    let color_row_all = vec![
            ColorRow::new(
                Rgba([0x3B, 0x43, 0x54, 255]),
                5,
                (2560.0/240.0,1440.0/19.0,2560.0/440.0,1440.0/89.0),
                InputMode::Keyboard,
            ),
            ColorRow::new(
                Rgba([0x4A, 0x52, 0x65, 255]),
                50,
                (2560.0/1156.0,1440.0/883.0,2560.0/1404.0,1440.0/925.0),
                InputMode::Mouse,
            ),
            ColorRow::new(
                Rgba([0x00, 0x00, 0x00, 255]),
                100,
                (2560.0/1180.0,1440.0/1200.0,2560.0/1400.0,1440.0/1267.0),
                InputMode::Mouse,
            ),
        ];
    let image = image::open(r"images\剧情-选择.png").unwrap().into_rgba8();
    let r = find_pixel_chat(&image,color_row_all);
    println!("{:?}",r.unwrap());
}
