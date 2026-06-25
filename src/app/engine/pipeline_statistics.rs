use wgpu::PipelineStatisticsTypes;

pub const NUM_QUERIES: u64 = 3;

pub struct QueryResults {
    pub vertex_shader_invocs: u64,
    pub clipper_invocs: u64,
    pub clipper_prims_out: u64,
}

pub struct Queries {
    pub set: wgpu::QuerySet,
    resolve_buf: wgpu::Buffer,
    destination_buf: wgpu::Buffer,
    pub results: QueryResults,
}

impl Queries {
    pub fn new(device: &wgpu::Device) -> Self {
        let set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("Pipeline Statistics Query Set"),
            ty: wgpu::QueryType::PipelineStatistics(
                PipelineStatisticsTypes::VERTEX_SHADER_INVOCATIONS
                    | PipelineStatisticsTypes::CLIPPER_INVOCATIONS
                    | PipelineStatisticsTypes::CLIPPER_PRIMITIVES_OUT,
            ),
            count: NUM_QUERIES as u32,
        });

        let resolve_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Query Resolve Buffer"),
            size: size_of::<u64>() as u64 * NUM_QUERIES,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let destination_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Query Destination Buffer"),
            size: size_of::<u64>() as u64 * NUM_QUERIES,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Self {
            set,
            resolve_buf,
            destination_buf,
            results: QueryResults {
                vertex_shader_invocs: 0,
                clipper_invocs: 0,
                clipper_prims_out: 0,
            },
        }
    }

    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.resolve_query_set(&self.set, 0..1, &self.resolve_buf, 0);
        encoder.copy_buffer_to_buffer(
            &self.resolve_buf,
            0,
            &self.destination_buf,
            0,
            self.resolve_buf.size(),
        );
    }

    pub fn poll_queries(&mut self, device: &wgpu::Device) {
        self.destination_buf
            .slice(..)
            .map_async(wgpu::MapMode::Read, |_| {});

        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();

        let calls: Vec<u64> = {
            let calls_view = self
                .destination_buf
                .slice(..(size_of::<u64>() as u64 * NUM_QUERIES))
                .get_mapped_range();
            bytemuck::allocation::pod_collect_to_vec(&calls_view)
        };

        self.destination_buf.unmap();

        self.results.vertex_shader_invocs = *calls.first().unwrap();
        self.results.clipper_invocs = *calls.get(1).unwrap();
        self.results.clipper_prims_out = *calls.get(2).unwrap();
    }
}
