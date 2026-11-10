/**
 * AudioWorkletProcessor for Low-Latency Emulation Audio
 * Runs on the dedicated audio rendering thread.
 */
class EmulatorAudioProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    // Capacity for ~0.25 seconds of audio @ 44.1kHz stereo
    this.bufferCapacity = 16384;
    this.bufferL = new Float32Array(this.bufferCapacity);
    this.bufferR = new Float32Array(this.bufferCapacity);
    this.writeIndex = 0;
    this.readIndex = 0;
    this.samplesAvailable = 0;

    this.port.onmessage = (event) => {
      const { type, data } = event.data;
      if (type === 'samples') {
        this.enqueueSamples(data);
      } else if (type === 'reset') {
        this.writeIndex = 0;
        this.readIndex = 0;
        this.samplesAvailable = 0;
      }
    };
  }

  enqueueSamples(interleavedSamples) {
    const pairCount = interleavedSamples.length / 2;
    for (let i = 0; i < pairCount; i++) {
      if (this.samplesAvailable >= this.bufferCapacity) {
        // Buffer full - drop oldest sample to avoid delay build-up
        this.readIndex = (this.readIndex + 1) % this.bufferCapacity;
        this.samplesAvailable--;
      }

      this.bufferL[this.writeIndex] = interleavedSamples[i * 2];
      this.bufferR[this.writeIndex] = interleavedSamples[i * 2 + 1];
      this.writeIndex = (this.writeIndex + 1) % this.bufferCapacity;
      this.samplesAvailable++;
    }
  }

  process(inputs, outputs, parameters) {
    const output = outputs[0];
    const channelL = output[0];
    const channelR = output[1];
    const quantumSize = channelL.length; // usually 128

    if (this.samplesAvailable >= quantumSize) {
      for (let i = 0; i < quantumSize; i++) {
        channelL[i] = this.bufferL[this.readIndex];
        channelR[i] = this.bufferR[this.readIndex];
        this.readIndex = (this.readIndex + 1) % this.bufferCapacity;
      }
      this.samplesAvailable -= quantumSize;
    } else {
      // Underflow: zero out channel
      channelL.fill(0);
      channelR.fill(0);
    }

    return true;
  }
}

registerProcessor('emulator-audio-processor', EmulatorAudioProcessor);
