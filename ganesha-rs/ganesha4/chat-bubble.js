class ChatBubble extends HTMLElement {
  constructor() {
    super();
    this.attachShadow({ mode: 'open' });
  }

  static get observedAttributes() {
    return ['text', 'alignment'];
  }

  attributeChangedCallback(name, oldValue, newValue) {
    if (name === 'text') {
      this.text = newValue;
    }
    if (name === 'alignment') {
      this.alignment = newValue;
    }
    this.render();
  }

  connectedCallback() {
    this.render();
  }

  render() {
    const alignmentClass = this.alignment === 'right' ? 'right' : 'left';

    this.shadowRoot.innerHTML = `
      <style>
        .chat-bubble {
          max-width: 70%;
          padding: 10px 15px;
          border-radius: 20px;
          margin-bottom: 10px;
          word-wrap: break-word;
        }

        .left {
          background-color: #e5e5ea;
          color: black;
          align-self: flex-start;
        }

        .right {
          background-color: #007bff;
          color: white;
          align-self: flex-end;
        }

        .container {
          display: flex;
          width: 100%;
        }

        .right-container {
          justify-content: flex-end;
          width: 100%;
          display: flex;
        }

        .left-container {
          justify-content: flex-start;
          width: 100%;
          display: flex;
        }
      </style>
      <div class="${alignmentClass === 'right' ? 'right-container' : 'left-container'}">
        <div class="chat-bubble ${alignmentClass}">
          ${this.text}
        </div>
      </div>
    `;
  }
}

customElements.define('chat-bubble', ChatBubble);
