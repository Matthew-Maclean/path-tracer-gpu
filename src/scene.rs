use crate::gpu::{run_shader, Camera, Triangle, Material};

#[derive(Clone, Debug)]
pub struct Scene
{
    pub camera: Camera,
    pub triangles: Vec<Triangle>,
    pub materials: Vec<Material>,
}

impl Scene
{
    pub fn new(pos: [f32; 3], front: [f32; 3], up: [f32; 3], fov: f32) -> Scene
    {
        Scene
        {
            camera: Camera
            {
                pos: pos,
                front: front,
                up: up,
                fov: fov,
            },
            triangles: Vec::new(),
            materials: Vec::new(),
        }
    }

    pub fn render(
        &self,
        res: [u32; 2],
        depth: u32,
        condition: &dyn Fn(u32) -> bool,
        debug: bool)
        -> image::RgbImage
    {
        let start = std::time::Instant::now();
        let mut image = Vec::with_capacity((res[0] * res[1]) as usize);

        let samples = run_shader(
            &mut image,
            res[0],
            res[1],
            self.camera,
            &self.triangles,
            &self.materials,
            depth,
            condition);

        let mut file = image::RgbImage::new(res[0], res[1]);

        for y in 0..res[1]
        {
            for x in 0..res[0]
            {
                let px = image[(y * res[0] + x) as usize];

                file.put_pixel(x, res[1] - y - 1, image::Rgb([
                    (px.r * 255.0 / samples as f32) as u8,
                    (px.g * 255.0 / samples as f32) as u8,
                    (px.b * 255.0 / samples as f32) as u8,
                ]));
            }
        }

        let time = std::time::Instant::now() - start;
        println!(
            "Finished {}x{} render with {} samples in {} ({:0.02}s/sample average)",
            res[0], res[1],
            samples,
            fmt_time(time),
            time.as_secs_f32() / samples as f32);

        if debug
        {
            add_debug_info(&mut file, self.triangles.len(), samples, time);
        }

        file
    }

    pub fn add_triangle(
        &mut self, a: [f32; 3], b: [f32; 3], c: [f32; 3], mat: u32)
        -> &mut Self
    {
        self.triangles.push(Triangle
        {
            a: a,
            b: b,
            c: c,
            mat: mat,
        });

        self
    }

    pub fn add_quad(
        &mut self, a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3], mat: u32)
        -> &mut Self
    {
        self
            .add_triangle(a, b, c, mat)
            .add_triangle(a, d, c, mat)
    }

    pub fn add_material(&mut self, mat: Material) -> u32
    {
        self.materials.push(mat);

        (self.materials.len() - 1) as u32
    }

    pub fn parse(s: &str) -> Result<Scene, String>
    {
        use json::JsonValue;

        use std::collections::HashMap;

        let top = json::parse(s)
            .map_err(|e| format!(
                "Error parsing scene JSON: {}", e))?;

        if !top.is_object()
        {
            return Err("Scene wasn't a JSON object".to_owned());
        }

        let mut scene = if top.has_key("camera")
        {
            let camera = &top["camera"];

            if !camera.is_object()
            {
                return Err("\"camera\" entry in Scene wasn't a object".to_owned());
            }

            let pos = if camera.has_key("pos")
            {
                parse_vec3(&camera["pos"], "camera", "pos")?
            }
            else
            {
                return Err("\"camera\" didn't contain \"pos\" array".to_owned());
            };

            let front = if camera.has_key("front")
            {
                parse_vec3(&camera["front"], "camera", "front")?
            }
            else
            {
                return Err("\"camera\" didn't contain \"front\" array".to_owned());
            };

            let up = if camera.has_key("up")
            {
                parse_vec3(&camera["up"], "camera", "up")?
            }
            else
            {
                return Err("\"camera\" didn't contain \"up\" array".to_owned());
            };

            let fov = if camera.has_key("fov")
            {
                let fov = &camera["fov"];
                if let Some(fov) = fov.as_f32()
                {
                    fov.to_radians()
                }
                else
                {
                    return Err("\"fov\" entry in \"camera\" wasn't an f32".to_owned());
                }
            }
            else
            {
                return Err("\"camera\" didn't contain \"fov\" f32".to_owned());
            };

            Scene::new(pos, front, up, fov)
        }
        else
        {
            return Err("Scene didn't contain \"camera\" object".to_owned());
        };

        let materials = if top.has_key("materials")
        {
            let mats = &top["materials"];

            if !mats.is_object()
            {
                return Err("\"materials\" entry in Scene wasn't an object".to_owned());
            }

            let mut map: HashMap<String, u32> = HashMap::new();

            for (name, mat) in mats.entries()
            {
                if map.contains_key(name)
                {
                    return Err(format!("Duplicate material \"{}\"", name));
                }

                let index = if mat.is_object()
                {
                    let colour = if mat.has_key("colour")
                    {
                        parse_vec3(&mat["colour"], name, "colour")?
                    }
                    else
                    {
                        [0.0, 0.0, 0.0]
                    };

                    let glow = if mat.has_key("glow")
                    {
                        parse_vec3(&mat["glow"], name, "glow")?
                    }
                    else
                    {
                        [0.0, 0.0, 0.0]
                    };

                    let gloss = if mat.has_key("gloss")
                    {
                        let gloss= &mat["gloss"];
                        if let Some(gloss) = gloss.as_f32()
                        {
                            gloss
                        }
                        else
                        {
                            return Err(format!(
                                "\"gloss\" entry in \"{}\" wasn't an f32", name));
                        }
                    }
                    else
                    {
                        0.0
                    };

                    let reflect_c = if  mat.has_key("reflect_c")
                    {
                        parse_vec3(&mat["reflect_c"], name, "reflect_c")?
                    }
                    else
                    {
                        [1.0, 1.0, 1.0]
                    };

                    scene.add_material(Material
                    {
                        colour: colour,
                        glow: glow,
                        gloss: gloss,
                        reflect_c: reflect_c,
                    })
                }
                else
                {
                    return Err(format!("Material \"{}\" wasn't an object", name));
                };

                map.insert(name.to_owned(), index);
            }

            map
        }
        else
        {
            return Err("Scene didn't contain \"materials\" object".to_owned());
        };

        if !top.has_key("surfaces")
        {
            return Err("Scene didn't contain \"surfaces\" array".to_owned());
        }

        let surfaces = &top["surfaces"];

        if !surfaces.is_array()
        {
            return Err("\"surfaces\" entry in Scene wasn't an array".to_owned());
        }

        for obj in surfaces.members()
        {
            if !obj.is_object()
            {
                return Err("surface wasn't an array".to_owned());
            }

            let mat = if obj.has_key("mat")
            {
                if let Some(mat) = obj["mat"].as_u32()
                {
                    mat
                }
                else if let Some(mat) = obj["mat"].as_str()
                {
                    if let Some(mat) = materials.get(mat)
                    {
                        *mat
                    }
                    else
                    {
                        return Err(format!("Unknown material \"{}\"", mat));
                    }
                }
                else
                {
                    return Err("\"mat\" entry in a surface wasn't a string or u32"
                        .to_owned());
                }
            }
            else
            {
                return Err("Surfaces didn't contain a \"mat\" index".to_owned());
            };

            if obj.has_key("tri")
            {
                if obj.has_key("quad")
                {
                    return Err("A surface cannot be a triangle and a quad".to_owned());
                }

                let tri = &obj["tri"];

                if !tri.is_array()
                {
                    return Err("A triangle was not an array of points".to_owned());
                }

                if tri.len() != 3
                {
                    return Err("A triangle list did not have length 3".to_owned());
                }

                let a = parse_vec3(&tri[0], "tri", "0")?;
                let b = parse_vec3(&tri[1], "tri", "1")?;
                let c = parse_vec3(&tri[2], "tri", "2")?;

                scene.add_triangle(a, b, c, mat);
            }
            else if obj.has_key("quad")
            {
                if obj.has_key("tri")
                {
                    return Err("A surface cannot be a triangle and a quad".to_owned());
                }

                let quad = &obj["quad"];

                if !quad.is_array()
                {
                    return Err("A quad was not an array of points".to_owned());
                }

                if quad.len() != 4
                {
                    return Err("A quad list did not have length 4".to_owned());
                }

                let a = parse_vec3(&quad[0], "quad", "0")?;
                let b = parse_vec3(&quad[1], "quad", "1")?;
                let c = parse_vec3(&quad[2], "quad", "2")?;
                let d = parse_vec3(&quad[3], "quad", "3")?;

                scene.add_quad(a, b, c, d, mat);
            }
            else
            {
                return Err("A surfaces wasn't a triangle or quad".to_owned());
            }
        }

        return Ok(scene);

        fn parse_vec3(val: &JsonValue, outer: &str, name: &str)
            -> Result<[f32; 3], String>
        {
            if !val.is_array()
            {
                return Err(format!("\"{}\" in \"{}\" wasn't an array",
                    name, outer))
            }

            if val.len() != 3
            {
                return Err(format!("\"{}\" in \"{}\" didn't have a length of 3",
                    name, outer));
            }

            let a = val[0].as_f32().ok_or(format!(
                "first value in \"{}\" wasn't an f32", name))?;
            let b = val[1].as_f32().ok_or(format!(
                "second value in \"{}\" wasn't an f32", name))?;
            let c = val[2].as_f32().ok_or(format!(
                "third value in \"{}\" wasn't an f32", name))?;

            Ok([a, b, c])
        }
    }
}

fn fmt_time(d: std::time::Duration) -> String
{
    let s = d.as_secs();

    format!("{}:{:02}:{:02}",
        s / 3600,
        (s % 3600) / 60,
        s % 60)
}

fn add_debug_info(
    image: &mut image::RgbImage,
    triangles: usize,
    samples: u32,
    time: std::time::Duration)
    -> bool
{
    let samples = format!("{} ", samples);
    let triangles = format!("{} ", triangles);
    let time = format!("{} ", fmt_time(time));

    let height = (3 * 8) + 1;
    let width = *[
        samples.len() + SAMPLES_TEXT[0].len(),
        triangles.len() + TRIANGLES_TEXT[0].len(),
        time.len() + TIME_TEXT[0].len()].iter().max().unwrap();

    if image.height() < height || image.width() < width as u32
    {
        return false;
    }

    let mut y_init = image.height() as usize - 3 * 8;

    for (val, text) in [
            (samples, SAMPLES_TEXT),
            (triangles, TRIANGLES_TEXT),
            (time, TIME_TEXT)].iter()
    {
        let mut x_init = 1;

        for c in val.chars()
        {
            let pat = NUMBERS_TEXT[match c
            {
                '0' => 0,
                '1' => 1,
                '2' => 2,
                '3' => 3,
                '4' => 4,
                '5' => 5,
                '6' => 6,
                '7' => 7,
                '8' => 8,
                '9' => 9,
                ':' => 10,
                ' ' => 11,
                _ => unreachable!(),
            }];

            for (y, line) in pat.iter().enumerate()
            {
                for (x, c) in line.chars().enumerate()
                {
                    image.put_pixel(
                        (x_init + x) as u32,
                        (y_init + y) as u32,
                        image::Rgb(if c == '#' { [255; 3] } else { [0; 3] }));
                }
            }

            x_init += pat[0].len();
        }

        for (y, line) in text.iter().enumerate()
        {
            for (x, c) in line.chars().enumerate()
            {
                image.put_pixel(
                    (x_init + x) as u32,
                    (y_init + y) as u32,
                    image::Rgb(if c == '#' { [255; 3] } else { [0; 3] }));
            }
        }

        y_init += text.len() + 1;
    }

    true
}

const SAMPLES_TEXT: [&'static str; 7] = [
    " ###   ###  #   # ####  #     #####  ### ",
    "#   # #   # ## ## #   # #     #     #   #",
    "#     #   # ## ## #   # #     #     #    ",
    " ###  ##### # # # ####  #     ###    ### ",
    "    # #   # #   # #     #     #         #",
    "#   # #   # #   # #     #     #     #   #",
    " ###  #   # #   # #     ##### #####  ### ",
//   ----- ----- ----- ----- ----- ----- -----
];

const TRIANGLES_TEXT: [&'static str; 7] = [
    "##### ####   ###   ###  #   #  ###  #     #####  ###",
    "  #   #   #   #   #   # ##  # #   # #     #     #   #",
    "  #   #   #   #   #   # ##  # #     #     #     #    ",
    "  #   ####    #   ##### # # # #  ## #     ####   ### ",
    "  #   #  #    #   #   # #  ## #   # #     #         #",
    "  #   #   #   #   #   # #  ## #   # #     #     #   #",
    "  #   #   #  ###  #   # #   #  ###  ##### #####  ### ",
//   ----- ----- ----- ----- ----- ----- ----- ----- -----
];

const TIME_TEXT: [&'static str; 7] = [
    "#####  ###  #   # #####",
    "  #     #   ## ## #    ",
    "  #     #   ## ## #    ",
    "  #     #   # # # #### ",
    "  #     #   #   # #    ",
    "  #     #   #   # #    ",
    "  #    ###  #   # #####",
];

const NUMBERS_TEXT: [[&'static str; 7]; 12] = [
    [
        " ###  ",
        "#   # ",
        "#  ## ",
        "# # # ",
        "##  # ",
        "#   # ",
        " ###  ",
    ],
    [
        "  #   ",
        " ##   ",
        "  #   ",
        "  #   ",
        "  #   ",
        "  #   ",
        " ###  ",
    ],
    [
        " ###  ",
        "#   # ",
        "    # ",
        "    # ",
        "   #  ",
        " ##   ",
        "##### ",
    ],
    [
        " ###  ",
        "#   # ",
        "    # ",
        "  ##  ",
        "    # ",
        "#   # ",
        " ###  ",
    ],
    [
        "   #  ",
        "  ##  ",
        " # #  ",
        "#  #  ",
        "##### ",
        "   #  ",
        "  ### ",
    ],
    [
        "##### ",
        "#     ",
        "###   ",
        "   #  ",
        "    # ",
        "#   # ",
        " ###  ",
    ],
    [
        " ###  ",
        "#   # ",
        "#     ",
        "####  ",
        "#   # ",
        "#   # ",
        " ###  ",
    ],
    [
        " ###  ",
        "#   # ",
        "    # ",
        "   #  ",
        "   #  ",
        "  #   ",
        "  #   ",
    ],
    [
        " ###  ",
        "#   # ",
        "#   # ",
        " ###  ",
        "#   # ",
        "#   # ",
        " ###  ",
    ],
    [
        " ###  ",
        "#   # ",
        "#   # ",
        " #### ",
        "    # ",
        "#   # ",
        " ###  ",
    ],
    [
        "      ",
        "  #   ",
        "      ",
        "      ",
        "      ",
        "  #   ",
        "      ",
    ],
    [
        "      ",
        "      ",
        "      ",
        "      ",
        "      ",
        "      ",
        "      ",
    ],
];
