use minifb::{Key, Window, WindowOptions};
use plotters::drawing::bitmap_pixel::BGRXPixel;
use plotters::prelude::*;
use regex::Regex;
use std::error::Error;
use std::io::BufRead;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::RwLock;
use std::thread;
use structopt::StructOpt;
 use std::{sync::atomic::{AtomicBool, Ordering}};
 use std::time::Duration;

#[derive(Debug, StructOpt)]
#[structopt(raw(setting = "structopt::clap::AppSettings::AllowNegativeNumbers"))]
struct Opt {
    #[structopt(short = "r", long = "regex")]
    /// This regex should have 1 or two groups that contain a number. This will be rendered into the graph.
    /// Check the capture option for more details
    regex: String,
	
	#[structopt(short = "t", long = "title")]
    /// Title of the window
    window_title: Option<String>,

	#[structopt(long = "x_min")]
    /// Minimum x coordinate for the graph
    x_min: Option<f64>,

    #[structopt(long = "y_min")]
    /// Minimum y coordinate for the graph
    ymin: Option<f64>,
    
    #[structopt(long = "x_max")]
    /// Maximum x coordinate for the graph
    xmax: Option<f64>,

    #[structopt(long = "y_max")]
    /// Maximum y coordinate for the graph
    ymax: Option<f64>,

	#[structopt(short = "c", long = "capture")]
    /// How the numbers will be captured from the regex groups.
    /// "1" to capture the first group as y and use the number lines that matched as x
    /// "-1" to capture the first group as y and use the number lines that matched as y
    /// "12" to capture two groups and use the first group as x and the second group as y
    /// "21" to capture two groups and use the first group as y and the second group as z
    capture_method: Option<String>,

    #[structopt(long = "reset_regex")]
    /// When a line matches this regex the current graph will be cleared
    reset_regex: Option<String>,
}

const W: usize = 680;
const H: usize = 320;

fn get_window_title(window_title: &str, _fx: f64, _fy: f64, _iphase: f64) -> String {
    format!("{}", window_title)
}

fn capture_with_method(line: &str, regex: &Regex, capture_method: &str, line_number: f64) -> Option<(f64, f64)> {
	         let caps = match regex.captures(&line) {
                Some(expr) => expr,
                None => return None,
            };
	    return match capture_method {
    	"1" => {
				let number = caps
                .get(1)
                .map_or(0f64, |m| f64::from_str(m.as_str()).unwrap_or(0f64));
                Some((line_number, number))

    	},
    	"-1" => {
				let number = caps
                .get(1)
                .map_or(0f64, |m| f64::from_str(m.as_str()).unwrap_or(0f64));
                Some((number, line_number))
    	}
    	"12" => {
    		if caps.len() != 2 {
    			println!("Error: requested that two groups but regex does not contain them");
    			return None
    		}

    		if caps.len() != 2 {
    			return None
    		}
    		let x = caps
                .get(1)
                .map_or(0f64, |m| f64::from_str(m.as_str()).unwrap_or(0f64));

			let y = caps
                .get(2)
                .map_or(0f64, |m| f64::from_str(m.as_str()).unwrap_or(0f64));

                Some((x, y))
    	}
    	"21" => {
    		if caps.len() != 2 {
    			println!("Error: requested that two groups but regex does not contain them");
    			return None
    		}

			let y = caps
                .get(1)
                .map_or(0f64, |m| f64::from_str(m.as_str()).unwrap_or(0f64));

			let x = caps
                .get(2)
                .map_or(0f64, |m| f64::from_str(m.as_str()).unwrap_or(0f64));

                Some((x, y))

    	}
    	_ => {
    		// At this point this should not happen.
    		println!("Error: Unknown capture method. {:?}", capture_method);
    		None
    	},
    };
}

fn create_reader_thread(still_running: Arc<AtomicBool>, regex: Regex, reset_regex: Option<Regex>, capture_method: String) -> (thread::JoinHandle<()>, Arc<RwLock<Vec<(f64, f64)>>>) {
    let data = Arc::new(RwLock::new(Vec::<(f64, f64)>::new()));
    let tdata = data.clone();
    let thread = thread::spawn(move || {
        println!("Starting reader thread.");

        let mut line_number = 0f64;
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {

            let line = line.unwrap();
			
			let captured = capture_with_method(&line, &regex, &capture_method, line_number);

            if let Some(captured) = captured {
				line_number += 1f64;
            	println!("x, y = {} {} -> {:?}", captured.0, captured.1, line);
            	tdata.write().unwrap().push((captured.0, captured.1));	
            }

           	if let Some(pattern) = &reset_regex {
           		if pattern.is_match(&line) {
           			println!("Clearing -> {}", line);
           			tdata.write().unwrap().clear();			
           		}
           	}
            
            if !still_running.load(Ordering::SeqCst) {
            	println!("Will stop reading from stdin");
            	break;
            }
        }
    });

    return (thread, data);
}

fn main() -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_args();
    let window_title = opt.window_title.unwrap_or("Grapher".to_string());
    let x_min = opt.x_min.unwrap_or(0f64);
    let ymin = opt.ymin.unwrap_or(0f64);
    let xmax = opt.xmax.unwrap_or(100f64);
    let ymax = opt.ymax.unwrap_or(100f64);
    let capture_method = opt.capture_method.unwrap_or("1".to_string());

    let regex = if let Ok(pattern) = Regex::new(&opt.regex) {
                pattern
            } else {
                println!("Could not read regex: {:?}", opt.regex);
                return Ok(()); // TODO replace with error
            };

    let reset_regex = match &opt.reset_regex {
    	Some(expr) => {
			let reset = if let Ok(pattern) = Regex::new(&expr) {
                pattern
            } else {
                println!("Could not read reset regex: {:?}", opt.reset_regex);
                return Ok(()); // TODO replace with error
            };

            Some(reset)
    	},
    	None => {
    		None
    	},
    };

    let capture_method = match capture_method.as_str() {
    	"1" => {
    		println!("Using first group as y and the number lines that matched as x");
    		capture_method
    	},
    	"-1" => {
    		println!("Using first group as y and the number lines that matched as y");
    		capture_method
    	}
    	"12" => {
    		println!("Using first group as x and second group as y");
    		capture_method
    	}
    	"21" => {
    		println!("Using first group as y and second group as x");
    		capture_method
    	}
    	method => {
    		println!("Error: parsing capture method. {:?}", method);
    		return Ok(()); // TODO replace with error
    	},
    };

    let mut buf = vec![0u8; W * H * 4];

    let mut window = Window::new(
        &get_window_title(&window_title, 0f64, 0f64, 0f64),
        W,
        H,
        WindowOptions::default(),
    )?;

    // Initial setup for graph
    let root =
        BitMapBackend::<BGRXPixel>::with_buffer_and_format(&mut buf[..], (W as u32, H as u32))?
            .into_drawing_area();
    root.fill(&BLACK)?;

    let mut chart = ChartBuilder::on(&root)
        .margin(10)
        .set_all_label_area_size(30)
        .build_ranged(x_min..xmax, ymin..ymax)?;

    chart
        .configure_mesh()
        .label_style(("sans-serif", 11).into_font().color(&GREEN))
        .axis_style(&GREEN)
        .draw()?;

    let cs = chart.into_chart_state();

    drop(root);

    // Update it first so we have a window with content even if stdin blocks because no input was provided
    window.update_with_buffer(unsafe { std::mem::transmute(&buf[..]) })?;

    // Start thread that reads and parses input
    let still_running = Arc::new(AtomicBool::new(true));
    let (thread, data) = create_reader_thread(still_running.clone(), regex, reset_regex, capture_method);

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let root =
            BitMapBackend::<BGRXPixel>::with_buffer_and_format(&mut buf[..], (W as u32, H as u32))?
                .into_drawing_area();
        let mut chart = cs.clone().restore(&root);
        chart.plotting_area().fill(&BLACK)?;

        chart
            .configure_mesh()
            .line_style_1(&GREEN.mix(0.2))
            .line_style_2(&TRANSPARENT)
            .draw()?;

        let current_data = data.read().unwrap();
        chart.draw_series(
            current_data
                .iter()
                .zip(current_data.iter().skip(1))
                .map(|(&(x0, y0), &(x1, y1))| PathElement::new(vec![(x0, y0), (x1, y1)], &GREEN)),
        )?;

        drop(current_data);
        drop(root);
        drop(chart);

        thread::sleep(Duration::from_millis(32)); // TODO Implement consistent framerate
        window.update_with_buffer(unsafe { std::mem::transmute(&buf[..]) })?;
    }

    println!("Exiting");

    still_running.store(false, Ordering::SeqCst);

    // Exit after a while if the reader thread doesn't ever finish (it isn't receiving input from stdin and it's blocked there)
    thread::spawn(|| {
    	println!("Waiting");
    	thread::sleep(Duration::new(3,0));
    	println!("Bye");
    	std::process::exit(0);
	});

    println!("Waiting for reader thread");
    thread.join().unwrap();
    println!("Bye");

    Ok(())
}
