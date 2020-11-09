use crossbeam_channel::Receiver;
use std::collections::VecDeque;

struct SingleLocatedSource<S: rodio::Sample> {
    source: Box<dyn rodio::Source<Item = S>>,
    position: [f32; 3],
    attenuation_rate: f32,
    amplify: f32,
    doppler: bool,
    msg_channel: Receiver<SingleLocatedSourceMsg>,

    curr_listener_pos: [f32; 3],
    curr_delay_atten: Vec<(usize, f32)>,
}

enum SingleLocatedSourceMsg {
    Move([f32; 3]),
    SetDoppler(bool),
    SetAmplify(f32),
}

pub struct LocatedSources<S: rodio::Sample> {
    sources: Vec<SingleLocatedSource<S>>,
    sample_rate: u32,

    delay_buffer: Vec<VecDeque<S>>,
    sound_velocity: f32,

    current_processing_channel: usize,
    output_channels: Vec<[f32; 3]>,

    msg_channel: Receiver<LocatedSourcesMsg<S>>,
}

enum LocatedSourcesMsg<S: rodio::Sample> {
    MoveListener(Vec<[f32; 3]>),
    AddSource(SingleLocatedSource<S>),
}

impl<S: rodio::Sample> Iterator for LocatedSources<S> {
    type Item = S;

    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        if self.current_processing_channel == 0 {
            // Update status from main thread
            let mut updated_pos = false;
            while let Ok(msg) = self.msg_channel.try_recv() {
                match msg {
                    LocatedSourcesMsg::MoveListener(new_pos) => {
                        if new_pos.len() == self.output_channels.len() {
                            updated_pos = true;
                            self.output_channels = new_pos;
                        }
                    }
                    LocatedSourcesMsg::AddSource(source) => {
                        if source.source.sample_rate() == self.sample_rate {
                            source.update_listener(&self.output_channels, self.sound_velocity);
                            source.update();
                            self.sources.push(source);
                        }
                    }
                }
            }

            // Prepare next audio frame
            for i in (0..self.sources.len()).rev() {
                if updated_pos {
                    self.sources[i].update_listener(&self.output_channels, self.sound_velocity)
                }
                self.sources[i].update();
                match self.sources[i].source.next() {
                    None => {
                        self.sources.swap_remove(i);
                    }
                    Some(sample) => {
                        self.sources[i]
                            .curr_delay_atten
                            .iter()
                            .for_each(|(delay, atten)| {
                                self.insert_channel(i, *delay, sample.amplify(*atten))
                            })
                    }
                }
            }
        }

        // Output the current frame
        let sample = self.pop_channel(self.current_processing_channel);
        self.current_processing_channel += 1;
        if self.current_processing_channel == self.output_channels.len() {
            self.current_processing_channel = 0;
        }

        Some(sample)
    }
}

impl<S> LocatedSources<S>
where
    S: rodio::Sample,
{
    fn insert_channel(&mut self, channel: usize, delay: usize, sample: S) {
        if let Some(buffer) = self.delay_buffer.get_mut(channel) {
            if let Some(target_sample) = buffer.get_mut(delay) {
                *target_sample = target_sample.saturating_add(sample);
            } else {
                buffer.resize(delay, S::zero_value());
                buffer.push_back(sample);
            }
        } else {
            let mut new_buffer = VecDeque::with_capacity(delay + 1);
            new_buffer.resize(delay, S::zero_value());
            new_buffer.push_back(sample);
            self.delay_buffer.resize(channel + 1, new_buffer);
        }
    }

    fn pop_channel(&mut self, channel: usize) -> S {
        self.delay_buffer
            .get_mut(channel)
            .and_then(VecDeque::pop_front)
            .unwrap_or_else(S::zero_value)
    }
}

fn euclidian_distance(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

impl<S> SingleLocatedSource<S>
where
    S: rodio::Sample,
{
    fn update_listener(&mut self, new_pos: &[[f32; 3]], sound_velocity: f32) {
        for i in 0..new_pos.len() {
            let distance = euclidian_distance(&new_pos[i], &self.position);
            let delay = if self.doppler {
                (distance * sound_velocity * self.source.sample_rate() as f32) as usize
            } else {
                0
            };

            let attenuation = self.amplify / (distance * self.attenuation_rate);
        }
    }

    fn update(&mut self) {}
}
