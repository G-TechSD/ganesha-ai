class Pagination extends HTMLElement {
  constructor() {
    super();
    this.shadow = this.attachShadow({ mode: 'open' });
    this.currentPage = 1;
    this.itemsPerPage = 10;
    this.totalItems = 0;
    this.render();
  }

  static get observedAttributes() {
    return ['total-items', 'items-per-page', 'current-page'];
  }

  attributeChangedCallback(name, oldValue, newValue) {
    if (name === 'total-items') {
      this.totalItems = parseInt(newValue, 10) || 0;
    } else if (name === 'items-per-page') {
      this.itemsPerPage = parseInt(newValue, 10) || 10;
    } else if (name === 'current-page') {
      this.currentPage = parseInt(newValue, 10) || 1;
    }
    this.render();
  }

  get totalPages() {
    return Math.ceil(this.totalItems / this.itemsPerPage);
  }

  render() {
    this.shadow.innerHTML = `
      <style>
        .pagination {
          display: flex;
          justify-content: center;
          align-items: center;
          padding: 10px;
        }

        .pagination button {
          background-color: #f2f2f2;
          border: 1px solid #ddd;
          color: #333;
          padding: 8px 12px;
          text-decoration: none;
          cursor: pointer;
          margin: 0 3px;
          border-radius: 4px;
        }

        .pagination button:hover {
          background-color: #ddd;
        }

        .pagination button:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }

        .pagination .current-page {
          font-weight: bold;
        }
      </style>
      <div class="pagination">
        <button id="prev" ?disabled="${this.currentPage <= 1}" @click="${this.previousPage.bind(this)}">Previous</button>
        <span>
          Page <span class="current-page">${this.currentPage}</span> of ${this.totalPages}
        </span>
        <button id="next" ?disabled="${this.currentPage >= this.totalPages}" @click="${this.nextPage.bind(this)}">Next</button>
      </div>
    `;

    this.shadow.querySelector('#prev').addEventListener('click', this.previousPage.bind(this));
    this.shadow.querySelector('#next').addEventListener('click', this.nextPage.bind(this));
  }

  previousPage() {
    if (this.currentPage > 1) {
      this.currentPage--;
      this.dispatchEvent(new CustomEvent('page-changed', { detail: this.currentPage }));
      this.render();
    }
  }

  nextPage() {
    if (this.currentPage < this.totalPages) {
      this.currentPage++;
      this.dispatchEvent(new CustomEvent('page-changed', { detail: this.currentPage }));
      this.render();
    }
  }
}

customElements.define('pagination-component', Pagination);
