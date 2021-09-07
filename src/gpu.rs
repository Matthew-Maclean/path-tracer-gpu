use wgpu::
{
    Instance,
    Backends,
    DeviceType,

    ComputePassDescriptor,
    ComputePipelineDescriptor,

    CommandEncoderDescriptor,

    ShaderSource,
    ShaderModuleDescriptor,

    BindGroupEntry,
    BindGroupDescriptor,

    BufferUsages,
    BufferDescriptor,

    util::
    {
        DeviceExt,

        BufferInitDescriptor,
    },
};
use pollster::block_on;
use bytemuck::cast_slice;

pub fn run_shader(
    image: &mut Vec<Colour>,
    width: u32,
    height: u32,
    camera: Camera,
    triangles: &[Triangle],
    materials: &[Material],
    depth: u32,
    condition: &dyn Fn(u32) -> bool)
    -> u32
{
    let instance = Instance::new(Backends::PRIMARY);

    let adapter = instance
        .enumerate_adapters(Backends::PRIMARY)
        .filter(|a| a.get_info().device_type == DeviceType::DiscreteGpu)
        .next()
        .unwrap();

    let (device, queue) = block_on(adapter
        .request_device(&Default::default(), None))
        .unwrap();

    let shader = device.create_shader_module(&ShaderModuleDescriptor
    {
        label: Some("compute"),
        source: ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });

    let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor
    {
        label: None,
        layout: None,
        module: &shader,
        entry_point: "main",
    });

    let info_buffer = device.create_buffer_init(&BufferInitDescriptor
    {
        label: Some("info buffer"),
        contents: cast_slice(&[Info
        {
            triangles: triangles.len() as u32,
            materials: materials.len() as u32,
            width: width,
            height: height,
            samples: 1,
            depth: depth,
        }]),
        usage: BufferUsages::UNIFORM,
    });

    let camera_buffer = device.create_buffer_init(&BufferInitDescriptor
    {
        label: Some("camera buffer"),
        contents: cast_slice(&[camera]),
        usage: BufferUsages::UNIFORM,
    });

    let triangle_buffer = device.create_buffer_init(&BufferInitDescriptor
    {
        label: Some("triangle buffer"),
        contents: cast_slice(triangles),
        usage: BufferUsages::STORAGE,
    });

    let material_buffer = device.create_buffer_init(&BufferInitDescriptor
    {
        label: Some("material buffer"),
        contents: cast_slice(materials),
        usage: BufferUsages::STORAGE,
    });

    let seed_buffer = device.create_buffer_init(&BufferInitDescriptor
    {
        label: Some("seed buffer"),
        contents: cast_slice(&[rand::random::<u32>()]),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
    });

    let image_size = std::mem::size_of::<Colour>() as u64
        * width as u64
        * height as u64;
    let image_buffer = device.create_buffer(&BufferDescriptor
    {
        label: Some("image buffer"),
        size: image_size,
        usage: BufferUsages::STORAGE
            | BufferUsages::COPY_SRC
            | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let staging_buffer = device.create_buffer(&BufferDescriptor
    {
        label: None,
        size: image_size,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bg_layout = pipeline.get_bind_group_layout(0);

    let bind_group = device.create_bind_group(&BindGroupDescriptor
    {
        label: None,
        layout: &bg_layout,
        entries: &[
            BindGroupEntry
            {
                binding: 0,
                resource: info_buffer.as_entire_binding(),
            },
            BindGroupEntry
            {
                binding: 1,
                resource: camera_buffer.as_entire_binding(),
            },
            BindGroupEntry
            {
                binding: 2,
                resource: image_buffer.as_entire_binding(),
            },
            BindGroupEntry
            {
                binding: 3,
                resource: triangle_buffer.as_entire_binding(),
            },
            BindGroupEntry
            {
                binding: 4,
                resource: material_buffer.as_entire_binding(),
            },
            BindGroupEntry
            {
                binding: 5,
                resource: seed_buffer.as_entire_binding(),
            },
        ]
    });

    let mut samples = 0;
    while condition(samples)
    {
        samples += 1;

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor
        {
            label: None,
        });

        queue.write_buffer(&seed_buffer, 0, cast_slice(&[rand::random::<u32>()]));

        {
            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor
            {
                label: None
            });
            cpass.set_pipeline(&pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch(width, height, 1);
        }

        encoder.copy_buffer_to_buffer(
            &image_buffer, 0,
            &staging_buffer, 0,
            image_size);

        queue.submit(Some(encoder.finish()));

        device.poll(wgpu::Maintain::Wait);
    }

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor
    {
        label: None,
    });

    encoder.copy_buffer_to_buffer(
        &image_buffer, 0,
        &staging_buffer, 0,
        image_size);

    queue.submit(Some(encoder.finish()));

    let buf_slice = staging_buffer.slice(..);
    let buf_future = buf_slice.map_async(wgpu::MapMode::Read);

    device.poll(wgpu::Maintain::Wait);

    if block_on(buf_future).is_err()
    {
        panic!("GPU Error!");
    }

    image.clear();
    let data = buf_slice.get_mapped_range();

    image.extend_from_slice(cast_slice::<u8, Colour>(&data));

    drop(data);
    staging_buffer.unmap();

    return samples;
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Info
{
    triangles: u32,
    materials: u32,
    width    : u32,
    height   : u32,
    samples  : u32,
    depth    : u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Colour
{
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Triangle
{
    pub a  : [f32; 3],
    pub b  : [f32; 3],
    pub c  : [f32; 3],
    pub mat: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Material
{
    pub colour   : [f32; 3],
    pub glow     : [f32; 3],
    pub gloss    : f32,
    pub reflect_c: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Camera
{
    pub pos  : [f32; 3],
    pub front: [f32; 3],
    pub up   : [f32; 3],
    pub fov  : f32,
}

unsafe impl bytemuck::Zeroable for Info { }
unsafe impl bytemuck::Pod for Info { }
unsafe impl bytemuck::Zeroable for Colour { }
unsafe impl bytemuck::Pod for Colour { }
unsafe impl bytemuck::Zeroable for Triangle { }
unsafe impl bytemuck::Pod for Triangle { }
unsafe impl bytemuck::Zeroable for Material { }
unsafe impl bytemuck::Pod for Material { }
unsafe impl bytemuck::Zeroable for Camera { }
unsafe impl bytemuck::Pod for Camera { }
