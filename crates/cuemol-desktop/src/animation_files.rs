#[cfg(feature = "egui-gui")]
pub(crate) use cuemol_command::movie::files::animation_ffmpeg_format_options;
pub(crate) use cuemol_command::movie::files::{
    animation_ffmpeg_args, animation_frame_count, animation_movie_output_path,
    cleanup_animation_outputs,
};
