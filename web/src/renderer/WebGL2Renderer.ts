const VERTEX_SHADER_SRC = `#version 300 es
in vec2 a_position;
in vec2 a_texCoord;
out vec2 v_texCoord;

void main() {
    gl_Position = vec4(a_position, 0.0, 1.0);
    v_texCoord = a_texCoord;
}
`;

const FRAGMENT_SHADER_SRC = `#version 300 es
precision highp float;

in vec2 v_texCoord;
out vec4 fragColor;

uniform sampler2D u_texture;
uniform vec2 u_resolution;      // Canvas output resolution
uniform vec2 u_textureSize;     // SNES source resolution (e.g. 256, 224)
uniform int u_crtEnabled;       // 1 = CRT on, 0 = Crisp Pixels
uniform float u_time;

// Barrel distortion curve
vec2 crtCurve(vec2 uv) {
    uv = (uv - 0.5) * 2.0;
    uv *= 1.0 + pow(length(uv) / 4.0, 2.0) * 0.15;
    return uv * 0.5 + 0.5;
}

void main() {
    if (u_crtEnabled == 0) {
        // Crisp nearest-neighbor pixel mode
        fragColor = texture(u_texture, v_texCoord);
        return;
    }

    // CRT Shader Pipeline
    vec2 uv = crtCurve(v_texCoord);

    // Bezel crop border
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        fragColor = vec4(0.02, 0.02, 0.03, 1.0);
        return;
    }

    // Base texture sample with subtle chromatic aberration
    float caOffset = 0.0012;
    float r = texture(u_texture, uv + vec2(-caOffset, 0.0)).r;
    float g = texture(u_texture, uv).g;
    float b = texture(u_texture, uv + vec2(caOffset, 0.0)).b;
    vec3 color = vec3(r, g, b);

    // Scanlines based on native SNES scanline count
    float scanline = sin(uv.y * u_textureSize.y * 3.14159265);
    scanline = (scanline * 0.5 + 0.5);
    color *= (0.75 + 0.25 * scanline);

    // Phosphor mask simulation (RGB triad)
    float mask = fract(gl_FragCoord.x * 0.3333);
    if (mask < 0.3333) {
        color *= vec3(1.05, 0.95, 0.95);
    } else if (mask < 0.6666) {
        color *= vec3(0.95, 1.05, 0.95);
    } else {
        color *= vec3(0.95, 0.95, 1.05);
    }

    // Subtle vignette
    float vignette = uv.x * uv.y * (1.0 - uv.x) * (1.0 - uv.y);
    vignette = clamp(pow(16.0 * vignette, 0.25), 0.0, 1.0);
    color *= vignette;

    fragColor = vec4(color, 1.0);
}
`;

export class WebGL2Renderer {
  private gl: WebGL2RenderingContext;
  private program!: WebGLProgram;
  private texture!: WebGLTexture;
  private vao!: WebGLVertexArrayObject;
  private width: number;
  private height: number;

  // Uniform locations
  private uResolutionLoc!: WebGLUniformLocation | null;
  private uTextureSizeLoc!: WebGLUniformLocation | null;
  private uCrtEnabledLoc!: WebGLUniformLocation | null;
  private uTimeLoc!: WebGLUniformLocation | null;

  public crtEnabled: boolean = true;

  constructor(canvas: HTMLCanvasElement, width: number = 256, height: number = 224) {
    this.width = width;
    this.height = height;

    const gl = canvas.getContext('webgl2', {
      alpha: false,
      antialias: false,
      depth: false,
      powerPreference: 'high-performance',
      preserveDrawingBuffer: false,
    });

    if (!gl) {
      throw new Error('WebGL2 is not supported in this browser.');
    }
    this.gl = gl;

    this.initGL();
  }

  private initGL(): void {
    const gl = this.gl;

    const vertShader = this.compileShader(gl.VERTEX_SHADER, VERTEX_SHADER_SRC);
    const fragShader = this.compileShader(gl.FRAGMENT_SHADER, FRAGMENT_SHADER_SRC);

    this.program = gl.createProgram()!;
    gl.attachShader(this.program, vertShader);
    gl.attachShader(this.program, fragShader);
    gl.linkProgram(this.program);

    if (!gl.getProgramParameter(this.program, gl.LINK_STATUS)) {
      throw new Error(`Shader Link Error: ${gl.getProgramInfoLog(this.program)}`);
    }

    gl.useProgram(this.program);

    // Uniforms
    this.uResolutionLoc = gl.getUniformLocation(this.program, 'u_resolution');
    this.uTextureSizeLoc = gl.getUniformLocation(this.program, 'u_textureSize');
    this.uCrtEnabledLoc = gl.getUniformLocation(this.program, 'u_crtEnabled');
    this.uTimeLoc = gl.getUniformLocation(this.program, 'u_time');

    // Create Full-screen Quad (X, Y, U, V)
    // Coords: [pos.x, pos.y, uv.u, uv.v]
    const quadVertices = new Float32Array([
      -1.0, -1.0,  0.0, 1.0, // bottom-left
       1.0, -1.0,  1.0, 1.0, // bottom-right
      -1.0,  1.0,  0.0, 0.0, // top-left
      -1.0,  1.0,  0.0, 0.0, // top-left
       1.0, -1.0,  1.0, 1.0, // bottom-right
       1.0,  1.0,  1.0, 0.0, // top-right
    ]);

    this.vao = gl.createVertexArray()!;
    gl.bindVertexArray(this.vao);

    const vbo = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
    gl.bufferData(gl.ARRAY_BUFFER, quadVertices, gl.STATIC_DRAW);

    const aPosLoc = gl.getAttribLocation(this.program, 'a_position');
    gl.enableVertexAttribArray(aPosLoc);
    gl.vertexAttribPointer(aPosLoc, 2, gl.FLOAT, false, 4 * Float32Array.BYTES_PER_ELEMENT, 0);

    const aTexLoc = gl.getAttribLocation(this.program, 'a_texCoord');
    gl.enableVertexAttribArray(aTexLoc);
    gl.vertexAttribPointer(aTexLoc, 2, gl.FLOAT, false, 4 * Float32Array.BYTES_PER_ELEMENT, 2 * Float32Array.BYTES_PER_ELEMENT);

    // Setup Texture for SNES Framebuffer
    this.texture = gl.createTexture()!;
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);

    // Allocate immutable storage or initial empty texture
    gl.texImage2D(
      gl.TEXTURE_2D,
      0,
      gl.RGBA8,
      this.width,
      this.height,
      0,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      null
    );
  }

  private compileShader(type: number, source: string): WebGLShader {
    const gl = this.gl;
    const shader = gl.createShader(type)!;
    gl.shaderSource(shader, source);
    gl.compileShader(shader);

    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      const info = gl.getShaderInfoLog(shader);
      gl.deleteShader(shader);
      throw new Error(`Shader compile error: ${info}`);
    }
    return shader;
  }

  /**
   * Uploads the pixel data directly from Wasm linear memory without any allocation or copy.
   */
  public renderFrame(pixelView: Uint8Array, timeSec: number): void {
    const gl = this.gl;

    // Viewport
    gl.viewport(0, 0, gl.canvas.width, gl.canvas.height);

    // 1. Upload Texture directly using subImage
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.texSubImage2D(
      gl.TEXTURE_2D,
      0,
      0,
      0,
      this.width,
      this.height,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      pixelView
    );

    // 2. Set Uniforms
    gl.useProgram(this.program);
    gl.uniform2f(this.uResolutionLoc, gl.canvas.width, gl.canvas.height);
    gl.uniform2f(this.uTextureSizeLoc, this.width, this.height);
    gl.uniform1i(this.uCrtEnabledLoc, this.crtEnabled ? 1 : 0);
    gl.uniform1f(this.uTimeLoc, timeSec);

    // 3. Draw
    gl.bindVertexArray(this.vao);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
  }
}
