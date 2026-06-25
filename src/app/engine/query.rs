const NUM_TIMESTAMP_QUERIES: u64 = 2;

pub struct QueryResults {
    pub vertex_shader_invocs: u64,
    pub clipper_invocs: u64,
    pub clipper_prims_out: u64,
    pub rpass_delta: u64,
}

impl QueryResults {
    pub fn store_data(&mut self, data: &[u64; 5]) {
        self.vertex_shader_invocs = data[0];
        self.clipper_invocs = data[1];
        self.clipper_prims_out = data[2];

        self.rpass_delta = data[4].wrapping_sub(data[3]);
    }
}

pub struct Queries {
    pub pipeline_stats_set: wgpu::QuerySet,
    pub timestamps_set: wgpu::QuerySet,
    resolve_buf: wgpu::Buffer,
    destination_buf: wgpu::Buffer,
    pub results: QueryResults,
}

impl Queries {
    pub fn new(device: &wgpu::Device) -> Self {
        let pipeline_stats_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("Pipeline Statistics Query Set"),
            ty: wgpu::QueryType::PipelineStatistics(
                wgpu::PipelineStatisticsTypes::VERTEX_SHADER_INVOCATIONS
                    | wgpu::PipelineStatisticsTypes::CLIPPER_INVOCATIONS
                    | wgpu::PipelineStatisticsTypes::CLIPPER_PRIMITIVES_OUT,
            ),
            count: 1,
        });

        let timestamps_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("Timestamp Query Set"),
            ty: wgpu::QueryType::Timestamp,
            count: NUM_TIMESTAMP_QUERIES as u32,
        });

        // Resolve buffers are aligned to 256 bits
        let resolve_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Query Resolve Buffer"),
            size: 256 + size_of::<u64>() as u64 * NUM_TIMESTAMP_QUERIES,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let destination_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Query Destination Buffer"),
            size: 256 + size_of::<u64>() as u64 * NUM_TIMESTAMP_QUERIES,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Self {
            pipeline_stats_set,
            timestamps_set,
            resolve_buf,
            destination_buf,
            results: QueryResults {
                vertex_shader_invocs: 0,
                clipper_invocs: 0,
                clipper_prims_out: 0,
                rpass_delta: 0,
            },
        }
    }

    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.resolve_query_set(&self.pipeline_stats_set, 0..1, &self.resolve_buf, 0);
        encoder.resolve_query_set(&self.timestamps_set, 0..2, &self.resolve_buf, 256);
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

        let calls_view = self.destination_buf.slice(..).get_mapped_range();
        let data: &[u64] = bytemuck::cast_slice(&calls_view);

        self.results
            .store_data(&[data[0], data[1], data[2], data[32], data[33]]);

        drop(calls_view);

        self.destination_buf.unmap();
    }
}
