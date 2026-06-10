class AmbientSynthEngine {
  private ctx: AudioContext | null = null;
  private activeOscs: { type: string; osc: OscillatorNode; gain: GainNode }[] = [];
  private mainGain: GainNode | null = null;
  private schedulerTimer: NodeJS.Timeout | null = null;
  private currentMode: string = '';

  constructor() {}

  private init() {
    if (!this.ctx) {
      const AudioCtx = window.AudioContext || (window as any).webkitAudioContext;
      if (AudioCtx) {
        this.ctx = new AudioCtx();
        this.mainGain = this.ctx.createGain();
        this.mainGain.gain.setValueAtTime(0.4, this.ctx.currentTime);
        this.mainGain.connect(this.ctx.destination);
      }
    }
    if (this.ctx && this.ctx.state === 'suspended') {
      this.ctx.resume();
    }
  }

  public setVolume(volume: number) {
    this.init();
    if (this.mainGain && this.ctx) {
      this.mainGain.gain.linearRampToValueAtTime(volume * 0.4, this.ctx.currentTime + 0.1);
    }
  }

  public start(mode: string) {
    this.init();
    if (!this.ctx) return;

    this.stop();
    this.currentMode = mode;

    switch (mode) {
      case 'star-rail':
        this.playStarRailSpace();
        break;
      case 'genshin-impact':
        this.playGenshinPeace();
        break;
      case 'zenless-zone-zero':
        this.playZZZBeats();
        break;
      default:
        break;
    }
  }

  public stop() {
    if (this.schedulerTimer) {
      clearInterval(this.schedulerTimer);
      this.schedulerTimer = null;
    }
    this.activeOscs.forEach(({ osc, gain }) => {
      try {
        osc.stop();
      } catch (e) {}
    });
    this.activeOscs = [];
  }

  // 1. Honkai Star Rail: Ambient Space Drone Synth
  private playStarRailSpace() {
    if (!this.ctx || !this.mainGain) return;
    const now = this.ctx.currentTime;

    // Deep pad 1
    const osc1 = this.ctx.createOscillator();
    const gain1 = this.ctx.createGain();
    osc1.type = 'triangle';
    osc1.frequency.setValueAtTime(110, now); // A2 note
    gain1.gain.setValueAtTime(0.08, now);
    osc1.connect(gain1);
    gain1.connect(this.mainGain);
    osc1.start(now);
    this.activeOscs.push({ type: 'star-rail', osc: osc1, gain: gain1 });

    // Cosmic shimmer
    const osc2 = this.ctx.createOscillator();
    const gain2 = this.ctx.createGain();
    osc2.type = 'sine';
    osc2.frequency.setValueAtTime(330, now); // E4 note
    gain2.gain.setValueAtTime(0.04, now);
    osc2.connect(gain2);
    gain2.connect(this.mainGain);
    
    // Slow volume sweep
    gain2.gain.setValueAtTime(0.01, now);
    gain2.gain.linearRampToValueAtTime(0.05, now + 3);
    
    osc2.start(now);
    this.activeOscs.push({ type: 'star-rail', osc: osc2, gain: gain2 });

    // Modulating space wind sound (with low-frequency LFO)
    let phase = 0;
    this.schedulerTimer = setInterval(() => {
      if (!this.ctx) return;
      phase += 0.1;
      const freq = 110 + Math.sin(phase) * 1.5;
      try {
        osc1.frequency.setValueAtTime(freq, this.ctx.currentTime);
      } catch (e) {}
    }, 150);
  }

  // 2. Genshin Impact: Pentatonic Harp Flute
  private playGenshinPeace() {
    if (!this.ctx || !this.mainGain) return;
    const now = this.ctx.currentTime;

    // Harmonic backdrop
    const oscBg = this.ctx.createOscillator();
    const gainBg = this.ctx.createGain();
    oscBg.type = 'sine';
    oscBg.frequency.setValueAtTime(196, now); // G3 note
    gainBg.gain.setValueAtTime(0.06, now);
    oscBg.connect(gainBg);
    gainBg.connect(this.mainGain);
    oscBg.start(now);
    this.activeOscs.push({ type: 'genshin', osc: oscBg, gain: gainBg });

    // Random Harp Arpeggio Melodies
    const pentatonicNotes = [261.63, 293.66, 329.63, 392.00, 440.00, 523.25]; // C, D, E, G, A, C5 (Pentatonic major)
    let noteIndex = 0;

    this.schedulerTimer = setInterval(() => {
      if (!this.ctx || !this.mainGain) return;
      const t = this.ctx.currentTime;
      
      // select standard pleasant melody step
      const randomPitchIndex = Math.floor(Math.random() * pentatonicNotes.length);
      const pitch = pentatonicNotes[randomPitchIndex];

      const oscNote = this.ctx.createOscillator();
      const gainNote = this.ctx.createGain();
      oscNote.type = 'sine';
      oscNote.frequency.value = pitch;
      
      // Decay envelope
      gainNote.gain.setValueAtTime(0, t);
      gainNote.gain.linearRampToValueAtTime(0.12, t + 0.1);
      gainNote.gain.exponentialRampToValueAtTime(0.001, t + 2.0);

      oscNote.connect(gainNote);
      gainNote.connect(this.mainGain);
      oscNote.start(t);
      oscNote.stop(t + 2.1);

      // Clean reference older expired notes
      setTimeout(() => {
        try {
          oscNote.disconnect();
          gainNote.disconnect();
        } catch (e) {}
      }, 2500);

    }, 1800);
  }

  // 3. Zenless Zone Zero: Cyber lofi beats
  private playZZZBeats() {
    if (!this.ctx || !this.mainGain) return;
    const now = this.ctx.currentTime;

    // Deep tech Bass bassline
    const bass = this.ctx.createOscillator();
    const bassGain = this.ctx.createGain();
    bass.type = 'sine';
    bass.frequency.setValueAtTime(82.4, now); // E2 note
    bassGain.gain.setValueAtTime(0.15, now);
    bass.connect(bassGain);
    bassGain.connect(this.mainGain);
    bass.start(now);
    this.activeOscs.push({ type: 'zzz', osc: bass, gain: bassGain });

    // Cool beat synthesizer loop
    let beatStep = 0;
    this.schedulerTimer = setInterval(() => {
      if (!this.ctx || !this.mainGain) return;
      const t = this.ctx.currentTime;

      // 1. Synth kick sound
      if (beatStep % 4 === 0) {
        const kick = this.ctx.createOscillator();
        const kickGain = this.ctx.createGain();
        kick.type = 'sine';
        kick.frequency.setValueAtTime(120, t);
        kick.frequency.exponentialRampToValueAtTime(45, t + 0.12);
        
        kickGain.gain.setValueAtTime(0.35, t);
        kickGain.gain.exponentialRampToValueAtTime(0.001, t + 0.15);

        kick.connect(kickGain);
        kickGain.connect(this.mainGain);
        kick.start(t);
        kick.stop(t + 0.2);
      }

      // 2. Tech hi hat snare click
      if (beatStep % 2 === 1) {
        const snare = this.ctx.createOscillator();
        const snareGain = this.ctx.createGain();
        snare.type = 'triangle';
        snare.frequency.setValueAtTime(800, t);
        
        snareGain.gain.setValueAtTime(0.03, t);
        snareGain.gain.exponentialRampToValueAtTime(0.001, t + 0.08);

        snare.connect(snareGain);
        snareGain.connect(this.mainGain);
        snare.start(t);
        snare.stop(t + 0.1);
      }

      beatStep = (beatStep + 1) % 8;
    }, 380);
  }
}

export const soundEngine = new AmbientSynthEngine();
