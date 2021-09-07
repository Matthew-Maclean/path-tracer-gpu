use clap::{App, Arg};

mod gpu;
mod scene;

use scene::Scene;

fn main()
{
    let matches = App::new("GPU Path Tracer")
        .version("1.0")
        .about("A path tracer on the GPU")
        .arg(Arg::with_name("scene")
            .short("s")
            .long("scene")
            .help("The scene to render")
            .value_name("SCENE")
            .takes_value(true)
            .required(true))
        .arg(Arg::with_name("output")
            .short("o")
            .long("output")
            .help("The file to render to")
            .value_name("OUTPUT")
            .takes_value(true)
            .required(true))
        .arg(Arg::with_name("resolution")
            .short("r")
            .long("resolution")
            .help("The resolution of the render, as width:height")
            .value_name("RESOLUTION")
            .takes_value(true)
            .required(true))
        .arg(Arg::with_name("max-samples")
            .short("m")
            .long("max-samples")
            .help("The maximum number of samples to process")
            .value_name("SAMPLES")
            .takes_value(true))
        .arg(Arg::with_name("time-limit")
            .short("t")
            .long("time-limit")
            .help("The maximum number of time to render for, as h:m:s")
            .value_name("TIME")
            .takes_value(true))
        .arg(Arg::with_name("progressive")
            .short("p")
            .long("progressive")
            .help("Perform a progressive render that will continue until stopped"))
        .arg(Arg::with_name("debug")
            .short("d")
            .long("debug")
            .help("Add information about the scene and render to image"))
        .get_matches();

    let file = std::fs::read_to_string(
        matches.value_of("scene").unwrap()).unwrap();

    let scene = match Scene::parse(&file)
    {
        Ok(s) => s,
        Err(e) =>
        {
            println!("Error: {}", e);
            return;
        }
    };

    let output = matches.value_of("output").unwrap();

    {
        if let Err(e) = std::fs::File::create(output)
        {
            println!("Error: {}", e);
        }
    }

    let res = match parse_resolution(matches.value_of("resolution").unwrap())
    {
        Ok(res) => res,
        Err(e) =>
        {
            println!("Error: {}", e);
            return;
        },
    };

    let (samples, def_samples) = match matches.value_of("max-samples")
    {
        Some(s) => match s.trim().parse::<u32>()
        {
            Ok(s) => (s, false),
            Err(_) =>
            {
                println!("Error: Could not parse maximum samples");
                return;
            },
        }
        None => (100_000, true),
    };

    let time = match matches.value_of("time-limit")
    {
        Some(t) => Some(match parse_time(t)
        {
            Ok(t) => t,
            Err(e) =>
            {
                println!("Error: {}", e);
                return;
            },
        }),
        None => None,
    };

    let p = matches.is_present("progressive");
    let debug = matches.is_present("debug");

    print_intro(res, samples, def_samples, time, p);

    let image = if p
    {
        scene.render(res, 5, &progressive(samples, time), debug)
    }
    else if let Some(time) = time
    {
        scene.render(res, 5, &time_limit(samples, time), debug)
    }
    else
    {
        scene.render(res, 5, &samples_limit(samples), debug)
    };

    image.save(output).unwrap();
}

fn samples_limit(max: u32) -> impl Fn(u32) -> bool
{
    move |samples| samples < max
}

fn time_limit(max: u32, time: std::time::Duration) -> impl Fn(u32) -> bool
{
    let start = std::time::Instant::now();

    move |samples| samples < max && std::time::Instant::now() - start < time
}

fn progressive(max: u32, time: Option<std::time::Duration>) -> impl Fn(u32) -> bool
{
    use std::sync::*;

    let start = std::time::Instant::now();

    let flag = Arc::new(atomic::AtomicBool::new(true));
    let flag_c = flag.clone();

    std::thread::spawn(move ||
    {
        use std::io::{self, BufRead};

        println!("Progressive Render: enter 's' or 'S' to stop the render.");

        let stdin = io::stdin();
        for line in stdin.lock().lines()
        {
            if let Ok(line) = line
            {
                if line.trim().to_lowercase() == "s"
                {
                    flag.store(true, atomic::Ordering::Relaxed);
                }
            }
        }
    });

    move |samples|
    {
        if samples >= max
        {
            return false;
        }

        if let Some(time) = time
        {
            if std::time::Instant::now() - start > time
            {
                return false;
            }
        }

        if !flag_c.load(atomic::Ordering::Relaxed)
        {
            return false;
        }

        true
    }
}

fn parse_resolution(res: &str) -> Result<[u32; 2], String>
{
    let mut split = res.split(":");
    let w = split.next()
        .ok_or("Could not parse resolution".to_owned())?
        .trim()
        .parse::<u32>()
        .map_err(|_| "Could not parse resolution width".to_owned())?;
    let h = split.next()
        .ok_or("Could not parse resolution height".to_owned())?
        .trim()
        .parse::<u32>()
        .map_err(|_| "Could not parse resolution height".to_owned())?;

    Ok([w, h])
}

fn parse_time(time: &str) -> Result<std::time::Duration, String>
{
    let mut split = time.split(":");
    let first = split.next()
        .ok_or("Could not parse time-limit".to_owned())?
        .trim()
        .parse::<u64>()
        .map_err(|_| "Could not parse time-limit".to_owned())?;
    let second = if let Some(second) = split.next()
    {
        Some(second.trim()
            .parse::<u64>()
            .map_err(|_| "Could not parse time-limit".to_owned())?)
    } else { None };
    let third = if let Some(third) = split.next()
    {
        Some(third.trim()
            .parse::<u64>()
            .map_err(|_| "Could not parse time-limit".to_owned())?)
    } else { None };

    Ok(std::time::Duration::from_secs(match (first, second, third)
    {
        (seconds, None, None) => seconds,
        (minutes, Some(seconds), None) => minutes * 60 + seconds,
        (hours, Some(minutes), Some(seconds))
            => hours * 3600 + minutes * 60 + seconds,
        _ => unreachable!()
    }))
}

fn print_intro(
    res: [u32; 2],
    samples: u32,
    def_samples: bool,
    time: Option<std::time::Duration>,
    progressive: bool)
{
    let samples = if def_samples
    {
        format!("{} (default) samples", samples)
    }
    else
    {
        format!("{} samples", samples)
    };

    if let Some(time) = time
    {
        let time =
        {
            let secs = time.as_secs();

            if secs >= 3600
            {
                format!("{}h:{}m:{}s ({}s)",
                    secs / 3600,
                    (secs % 3600) / 60,
                    secs % 60,
                    secs)
            }
            else if secs >= 60
            {
                format!("{}m:{}s ({}s)", secs / 60, secs % 60, secs)
            }
            else
            {
                format!("{}s", secs)
            }
        };

        if progressive
        {
            println!("Rendering at {}x{} progressively for {}, maximum {}",
                res[0], res[1],
                time,
                samples);
        }
        else
        {
            println!("Rendering at {}x{} for {}, maximum {}",
                res[0], res[1],
                time,
                samples);
        }
    }
    else
    {
        if progressive
        {
            println!("Rendering at {}x{} progressively, maximum {}",
                res[0], res[1],
                samples);
        }
        else
        {
            println!("Rendering at {}x{}, maximum {}",
                res[0], res[1],
                samples);
        }
    }
}
