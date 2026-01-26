class ProgressBar extends HTMLElement {
  constructor() {
    super();
    this.shadow = this.attachShadow({ mode: 'open' });
    this._progress = 0;
  }

  static get observedAttributes() {
    return ['progress'];
  }

  attributeChangedCallback(name, oldValue, newValue) {
    if (name === 'progress') {
      this.progress = newValue;
    }
  }

  get progress() {
    return this._progress;
  }

  set progress(value) {
    this._progress = value;
    this.updateProgress();
  }

  connectedCallback() {
    this.render();
  }

  render() {
    this.shadow.innerHTML = `
      <style>
        .progress-bar-container {
          width: 100%;
          height: 20px;
          background-color: #eee;
          border-radius: 10px;
          overflow: hidden;
        }

        .progress-bar {
          height: 100%;
          background-color: #4CAF50;
          width: 0%;
          transition: width 0.3s ease-in-out;
          border-radius: 10px;
        }
      </style>
      <div class="progress-bar-container">
        <div class="progress-bar"></div>
      </div>
    `;
    this.updateProgress();
  }

  updateProgress() {
    const progressBar = this.shadow.querySelector('.progress-bar');
    if (progressBar) {
      progressBar.style.width = `${this._progress}%`;
    }
  }
}

customElements.define('progress-bar', ProgressBar);
