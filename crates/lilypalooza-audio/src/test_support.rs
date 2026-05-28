//! Test-support backend that does not touch the system audio device.

use knyst::{
    audio_backend::{AudioBackend, AudioBackendError},
    controller::Controller,
    graph::{Graph, RunGraph, RunGraphSettings},
    resources::Resources,
};

pub(crate) struct TestSupportBackend {
    sample_rate: usize,
    block_size: usize,
    num_outputs: usize,
    run_graph: Option<RunGraph>,
}

impl std::fmt::Debug for TestSupportBackend {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TestSupportBackend")
            .field("sample_rate", &self.sample_rate)
            .field("block_size", &self.block_size)
            .field("num_outputs", &self.num_outputs)
            .finish_non_exhaustive()
    }
}

impl TestSupportBackend {
    pub(crate) fn new(sample_rate: usize, block_size: usize, num_outputs: usize) -> Self {
        Self {
            sample_rate,
            block_size,
            num_outputs,
            run_graph: None,
        }
    }
}

impl AudioBackend for TestSupportBackend {
    fn start_processing_return_controller(
        &mut self,
        mut graph: Graph,
        resources: Resources,
        run_graph_settings: RunGraphSettings,
        error_handler: Box<dyn FnMut(knyst::KnystError) + Send + 'static>,
    ) -> Result<Controller, AudioBackendError> {
        if self.run_graph.is_some() {
            return Err(AudioBackendError::BackendAlreadyRunning);
        }

        let (run_graph, resources_command_sender, resources_command_receiver) =
            RunGraph::new(&mut graph, resources, run_graph_settings)?;
        let controller = Controller::new(
            graph,
            error_handler,
            resources_command_sender,
            resources_command_receiver,
        );
        self.run_graph = Some(run_graph);
        Ok(controller)
    }

    fn stop(&mut self) -> Result<(), AudioBackendError> {
        if self.run_graph.take().is_some() {
            Ok(())
        } else {
            Err(AudioBackendError::BackendNotRunning)
        }
    }

    fn sample_rate(&self) -> usize {
        self.sample_rate
    }

    fn block_size(&self) -> Option<usize> {
        Some(self.block_size)
    }

    fn native_output_channels(&self) -> Option<usize> {
        Some(self.num_outputs)
    }

    fn native_input_channels(&self) -> Option<usize> {
        Some(0)
    }
}
