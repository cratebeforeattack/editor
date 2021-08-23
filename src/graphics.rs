use miniquad::{
    BlendFactor, BlendState, BlendValue, BufferLayout, Context, Equation,
    Pipeline, PipelineParams, Shader, ShaderMeta, Texture, UniformBlockLayout,
    UniformDesc, UniformType, VertexAttribute, VertexFormat,
};
use realtime_drawing::VertexPos3UvColor;

pub fn create_pipeline(ctx: &mut Context) -> Pipeline {
    let vertex_shader = r#"#version 100
            attribute vec2 pos;
            attribute vec2 uv;
            attribute vec4 color;
            uniform vec2 ;
            uniform vec2 screen_size;
            varying lowp vec2 v_uv;
            varying lowp vec4 v_color;
            void main() {
                gl_Position = vec4((pos / screen_size * 2.0 - 1.0) * vec2(1.0, -1.0), 0, 1);
                v_uv = uv;
                v_color = color / 255.0;
            }"#;
    let fragment_shader = r#"#version 100
            varying lowp vec2 v_uv;
            varying lowp vec4 v_color;
            uniform sampler2D tex;
            void main() {
                gl_FragColor = v_color * texture2D(tex, v_uv);
            }"#;
    let shader = Shader::new(
        ctx,
        vertex_shader,
        fragment_shader,
        ShaderMeta {
            images: vec!["tex".to_owned()],
            uniforms: UniformBlockLayout {
                // describes struct ShaderUniforms
                uniforms: vec![UniformDesc::new("screen_size", UniformType::Float2)],
            },
        },
    )
        .unwrap();

    let pipeline = Pipeline::with_params(
        ctx,
        &[BufferLayout::default()],
        &[
            VertexAttribute::new("pos", VertexFormat::Float3),
            VertexAttribute::new("uv", VertexFormat::Float2),
            VertexAttribute::new("color", VertexFormat::Byte4),
        ],
        shader,
        PipelineParams {
            alpha_blend: Some(BlendState::new(
                Equation::Add,
                BlendFactor::Value(BlendValue::SourceAlpha),
                BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
            )),
            color_blend: Some(BlendState::new(
                Equation::Add,
                BlendFactor::Value(BlendValue::SourceAlpha),
                BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
            )),
            ..Default::default()
        },
    );
    pipeline
}