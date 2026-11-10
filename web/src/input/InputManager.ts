export const BUTTONS = {
  B: 1 << 0,
  Y: 1 << 1,
  SELECT: 1 << 2,
  START: 1 << 3,
  UP: 1 << 4,
  DOWN: 1 << 5,
  LEFT: 1 << 6,
  RIGHT: 1 << 7,
  A: 1 << 8,
  X: 1 << 9,
  L: 1 << 10,
  R: 1 << 11,
} as const;

export class InputManager {
  private keyboardMask: number = 0;
  private keyMap: Record<string, number> = {
    ArrowUp: BUTTONS.UP,
    KeyW: BUTTONS.UP,
    ArrowDown: BUTTONS.DOWN,
    KeyS: BUTTONS.DOWN,
    ArrowLeft: BUTTONS.LEFT,
    KeyA: BUTTONS.LEFT,
    ArrowRight: BUTTONS.RIGHT,
    KeyD: BUTTONS.RIGHT,

    KeyZ: BUTTONS.B,
    KeyK: BUTTONS.B,

    KeyX: BUTTONS.A,
    KeyL: BUTTONS.A,

    KeyC: BUTTONS.Y,
    KeyI: BUTTONS.Y,

    KeyV: BUTTONS.X,
    KeyO: BUTTONS.X,

    KeyQ: BUTTONS.L,
    KeyE: BUTTONS.R,

    Enter: BUTTONS.START,
    ShiftRight: BUTTONS.SELECT,
    ShiftLeft: BUTTONS.SELECT,
  };

  private activeGamepadName: string | null = null;

  constructor() {
    window.addEventListener('keydown', (e) => this.onKeyDown(e));
    window.addEventListener('keyup', (e) => this.onKeyUp(e));

    window.addEventListener('gamepadconnected', (e) => {
      console.log('Gamepad connected:', e.gamepad.id);
      this.activeGamepadName = e.gamepad.id;
    });

    window.addEventListener('gamepaddisconnected', () => {
      this.activeGamepadName = null;
    });
  }

  private onKeyDown(e: KeyboardEvent): void {
    const bit = this.keyMap[e.code];
    if (bit !== undefined) {
      this.keyboardMask |= bit;
      e.preventDefault();
    }
  }

  private onKeyUp(e: KeyboardEvent): void {
    const bit = this.keyMap[e.code];
    if (bit !== undefined) {
      this.keyboardMask &= ~bit;
      e.preventDefault();
    }
  }

  public getButtonMask(port: number = 0): number {
    let mask = this.keyboardMask;

    // Check Gamepad
    const gamepads = navigator.getGamepads ? navigator.getGamepads() : [];
    const gp = gamepads[port];
    if (gp && gp.connected) {
      this.activeGamepadName = gp.id;

      // Standard mapping:
      // buttons[0] = B (A on Xbox), buttons[1] = A (B on Xbox)
      // buttons[2] = Y (X on Xbox), buttons[3] = X (Y on Xbox)
      if (gp.buttons[0]?.pressed) mask |= BUTTONS.B;
      if (gp.buttons[1]?.pressed) mask |= BUTTONS.A;
      if (gp.buttons[2]?.pressed) mask |= BUTTONS.Y;
      if (gp.buttons[3]?.pressed) mask |= BUTTONS.X;

      if (gp.buttons[4]?.pressed) mask |= BUTTONS.L;
      if (gp.buttons[5]?.pressed) mask |= BUTTONS.R;

      if (gp.buttons[8]?.pressed) mask |= BUTTONS.SELECT;
      if (gp.buttons[9]?.pressed) mask |= BUTTONS.START;

      // D-Pad buttons
      if (gp.buttons[12]?.pressed) mask |= BUTTONS.UP;
      if (gp.buttons[13]?.pressed) mask |= BUTTONS.DOWN;
      if (gp.buttons[14]?.pressed) mask |= BUTTONS.LEFT;
      if (gp.buttons[15]?.pressed) mask |= BUTTONS.RIGHT;

      // Left analog stick fallback
      if (gp.axes[0] < -0.5) mask |= BUTTONS.LEFT;
      if (gp.axes[0] > 0.5) mask |= BUTTONS.RIGHT;
      if (gp.axes[1] < -0.5) mask |= BUTTONS.UP;
      if (gp.axes[1] > 0.5) mask |= BUTTONS.DOWN;
    }

    return mask;
  }

  public getGamepadStatus(): string {
    return this.activeGamepadName ? `🎮 ${this.activeGamepadName.slice(0, 24)}...` : '⌨️ Teclado Ativo';
  }
}
