struct Colour
{
    r: f32;
    g: f32;
    b: f32;
};

struct Triangle
{
    a  : array<f32, 3>;
    b  : array<f32, 3>;
    c  : array<f32, 3>;
    mat: u32;
};

struct Material
{
    colour   : array<f32, 3>;
    glow     : array<f32, 3>;
    gloss    : f32;
    reflect_c: array<f32, 3>;
};

[[block]]
struct Info
{
    triangles: u32;
    materials: u32;
    width    : u32;
    height   : u32;
    samples  : u32;
    depth    : u32;
};

[[block]]
struct Camera
{
    pos  : array<f32, 3>;
    front: array<f32, 3>;
    up   : array<f32, 3>;
    fov  : f32;
};

[[block]]
struct Image
{
    pixels: [[stride(12)]] array<Colour>;
};

[[block]]
struct Triangles
{
    data: [[stride(40)]] array<Triangle>;
};

[[block]]
struct Materials
{
    data: [[stride(40)]] array<Material>;
};

[[block]]
struct Seeds
{
    data: [[stride(4)]] array<u32>;
};


struct Random
{
    state: u32;
    latest: f32;
};

[[group(0), binding(0)]]
var<uniform> info: Info;
[[group(0), binding(1)]]
var<uniform> camera: Camera;
[[group(0), binding(2)]]
var<storage, read_write> image: Image;
[[group(0), binding(3)]]
var<storage, read> triangles: Triangles;
[[group(0), binding(4)]]
var<storage, read> materials: Materials;
[[group(0), binding(5)]]
var<storage, read> seeds: Seeds;

struct Ray
{
    start: vec3<f32>;
    vec: vec3<f32>;
};

fn _vec3(v: array<f32, 3>) -> vec3<f32>
{
    return vec3<f32>(v[0], v[1], v[2]);
}

fn xorshift(state: Random) -> Random
{
    var r: Random;
    var x: u32 = state.state;

    x = x ^ (x << u32(13));
    x = x ^ (x >> u32(17));
    x = x ^ (x << u32(5));

    r.state = x;
    r.latest = f32(x) / 4294967295.0; // x / max u32

    return r;
}

fn ray_vs_triangle(ray: Ray, triangle: Triangle) -> vec3<f32>
{
    var eps: f32 = 0.0001;
    var invalid: vec3<f32> = vec3<f32>(99999.0, 99999.0, 99999.0);

    var va: vec3<f32> = _vec3(triangle.a);
    var vb: vec3<f32> = _vec3(triangle.b);
    var vc: vec3<f32> = _vec3(triangle.c);

    var edge_1: vec3<f32> = vb - va;
    var edge_2: vec3<f32> = vc - va;

    var h: vec3<f32> = cross(ray.vec, edge_2);
    var a: f32 = dot(edge_1, h);

    if (a > -eps && a < eps)
    {
        return invalid;
    }

    var f: f32 = 1.0 / a;
    var s: vec3<f32> = ray.start - va;
    var u: f32 = f * dot(s, h);

    if (u < 0.0 || u > 1.0)
    {
        return invalid;
    }

    var q: vec3<f32> = cross(s, edge_1);
    var v: f32 = f * dot(ray.vec, q);

    if (v < 0.0 || (u + v) > 1.0)
    {
        return invalid;
    }

    let t: f32 = f * dot(edge_2, q);

    if (t > eps)
    {
        return ray.start + ray.vec * t;
    }
    else
    {
        return invalid;
    }
}

fn pos_normal(ray: Ray, triangle: Triangle) -> vec3<f32>
{
    var a: vec3<f32> = _vec3(triangle.a);
    var b: vec3<f32> = _vec3(triangle.b);
    var c: vec3<f32> = _vec3(triangle.c);

    var normal: vec3<f32> = normalize(cross(b - a, c - a));

    if (dot(ray.vec, normal) >= 0.0)
    {
        return -normal;
    }
    else
    {
        return normal;
    }
}

fn reflect_vec(incoming: vec3<f32>, normal: vec3<f32>) -> vec3<f32>
{
    var v: vec3<f32> = normalize(incoming);
    var n: vec3<f32> = normalize(normal);

    return normalize(v - n * 2.0 * dot(v, n));
}

fn cast_ray(ray: Ray, rand: Random) -> vec3<f32>
{
    var ray = ray;
    var rand = rand;

    var push: f32 = 0.001;

    var colour: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);
    var throughput: vec3<f32> = vec3<f32>(1.0, 1.0, 1.0);
    var weight: f32 = 1.0;

    for (var d: u32 = u32(0); d < info.depth; d = d + u32(1))
    {
        var min_dist: f32 = 99999.0;
        var point: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);
        var norm: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);
        var mat: Material;

        for (var i: u32 = u32(0); i < info.triangles; i = i + u32(1))
        {
            var p: vec3<f32> = ray_vs_triangle(ray, triangles.data[i]);

            var dist: f32 = length(p - ray.start);

            if (dist < min_dist)
            {
                min_dist = dist;
                point = p;
                norm = pos_normal(ray, triangles.data[i]);
                mat = materials.data[triangles.data[i].mat];
            }
        }

        if (min_dist > 1000.0)
        {
            break;
        }

        rand = xorshift(rand);
        if (rand.latest >= mat.gloss)
        {
            colour = colour + (throughput * (_vec3(mat.glow) * weight));
            throughput = throughput * (_vec3(mat.colour) * weight);

            ray.start = point + norm * push;

            rand = xorshift(rand);
            var x: f32 = rand.latest * 2.0 - 1.0;
            rand = xorshift(rand);
            var y: f32 = rand.latest * 2.0 - 1.0;
            rand = xorshift(rand);
            var z: f32 = rand.latest * 2.0 - 1.0;

            ray.vec = normalize(norm + normalize(vec3<f32>(x, y, z)));

            weight = dot(norm, ray.vec);
        }
        else
        {
            throughput = throughput * _vec3(mat.reflect_c);

            ray.start = point + norm * push;
            ray.vec = normalize(reflect_vec(ray.vec, -norm));

            weight = 1.0;
        }
    }

    return colour;
}

[[stage(compute), workgroup_size(1)]]
fn main([[builtin(workgroup_id)]] coords: vec3<u32>)
{
    var rand: Random;

    rand.state = seeds.data[coords.z];

    rand.state = rand.state ^ (coords.x | u32(1)) << u32(6);
    rand.state = rand.state ^ (coords.y | u32(1)) << u32(18);

    var mask: u32 = u32(31);
    var count: u32 = coords.x & mask;
    rand.state = (rand.state << count) | (rand.state >> ((~count + u32(1)) & mask));

    var count: u32 = coords.y & mask;
    rand.state = (rand.state >> count) | (rand.state << ((~count + u32(1)) & mask));

    rand.state = rand.state | u32(1);

    rand.latest = 0.0;

    rand = xorshift(rand);

    var x: f32 = f32(coords.x);
    var y: f32 = f32(coords.y);

    var x_step: f32 = 1.0 / f32(info.width);
    var y_step: f32 = 1.0 / f32(info.height);

    var ratio: f32 = f32(info.width) / f32(info.height);

    var dist: f32 = 0.5 / tan(camera.fov / 2.0);

    var up: vec3<f32> = normalize(_vec3(camera.up));
    var front: vec3<f32> = normalize(_vec3(camera.front));
    var right: vec3<f32> = normalize(cross(front, up));

    var x_offset: f32 = -0.5 + x_step * (x + 0.5);
    var y_offset: f32 = (-0.5 + y_step * (y + 0.5)) / ratio;

    rand = xorshift(rand);
    var rx: f32 = rand.latest - 0.5;
    rand = xorshift(rand);
    var ry: f32 = rand.latest - 0.5;

    var pos: vec3<f32> = _vec3(camera.pos);
    var pix: vec3<f32> = pos
        + (front * dist)
        + (right * (x_offset + rx * x_step))
        + (up * (y_offset + ry * y_step));

    var ray: Ray;
    ray.start = pos;
    ray.vec = normalize(pix - pos);

    var px: u32 = coords.y * info.width + coords.x;

    var c: vec3<f32> = cast_ray(ray, rand);

    image.pixels[px][0] = image.pixels[px][0] + c.x / f32(info.samples);
    image.pixels[px][1] = image.pixels[px][1] + c.y / f32(info.samples);
    image.pixels[px][2] = image.pixels[px][2] + c.z / f32(info.samples);

    rand = xorshift(rand);
}
