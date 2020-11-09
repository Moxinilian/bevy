use crate::{AudioSource, Decodable};
use bevy_asset::Handle;

pub struct Audio<P = AudioSource>
where
    P: Decodable,
{
    awaiting_asset: Option<Handle<P>>,
}

impl<P> From<Handle<P>> for Audio
where
    P: Decodable,
    <P as Decodable>::Decoder: rodio::Source + Send + Sync,
    <<P as Decodable>::Decoder as Iterator>::Item: rodio::Sample + Send + Sync,
{
    fn from(handle: Handle<P>) -> Audio {
        todo!()
    }
}
