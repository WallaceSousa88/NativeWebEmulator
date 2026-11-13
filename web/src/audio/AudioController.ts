export class AudioController {
  private ctx: AudioContext | null = null;
  private workletNode: AudioWorkletNode | null = null;
  private isMuted: boolean = false;
  private initialized: boolean = false;

  public async init(): Promise<void> {
    if (this.initialized) return;

    const AudioContextClass = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
    this.ctx = new AudioContextClass({
      sampleRate: 44100,
      latencyHint: 'interactive',
    });

    try {
      await this.ctx.audioWorklet.addModule('/audio/emulator-audio-processor.js');
      this.workletNode = new AudioWorkletNode(this.ctx, 'emulator-audio-processor');
      this.workletNode.connect(this.ctx.destination);
      this.initialized = true;
    } catch (err) {
      console.error('Failed to load AudioWorklet:', err);
    }
  }

  public async resume(): Promise<void> {
    if (this.ctx && this.ctx.state === 'suspended') {
      await this.ctx.resume();
    }
  }

  public pushSamples(interleavedSamples: Float32Array<ArrayBufferLike>): void {
    if (!this.initialized || !this.workletNode || this.isMuted) return;
    if (interleavedSamples.length === 0) return;

    // We copy or transfer the slice to avoid keeping the wasm buffer locked
    const copy = new Float32Array(interleavedSamples);
    this.workletNode.port.postMessage(
      { type: 'samples', data: copy },
      [copy.buffer] // Transfer ownership without copying!
    );
  }

  public toggleMute(): boolean {
    this.isMuted = !this.isMuted;
    return this.isMuted;
  }

  public get muted(): boolean {
    return this.isMuted;
  }
}
