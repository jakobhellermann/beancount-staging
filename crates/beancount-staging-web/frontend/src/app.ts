import { ApiClient } from "./api";

interface EditState {
  account: string;
  payee?: string;
  narration?: string;
}

const KEYBINDS = {
  focus: {
    payee: "p",
    narration: "n",
    account: "a",
  },
  prev: ["ArrowLeft", "h"],
  next: ["ArrowRight", "l"],
  commit: "Enter",
};

class StagingApp {
  private api = new ApiClient();
  private directives: import("./types").Directive[] = [];
  private currentIndex = 0;
  private availableAccounts: string[] = [];
  private editStates: Map<string, EditState> = new Map();

  private transactionEl: HTMLElement;
  private counterEl: HTMLElement;
  private commitBtn: HTMLButtonElement;
  private messageEl: HTMLElement;
  private prevBtn: HTMLButtonElement;
  private nextBtn: HTMLButtonElement;

  constructor() {
    this.transactionEl = document.getElementById("transaction")!;
    this.counterEl = document.getElementById("counter")!;
    this.commitBtn = document.getElementById("commit") as HTMLButtonElement;
    this.messageEl = document.getElementById("message")!;
    this.prevBtn = document.getElementById("prev") as HTMLButtonElement;
    this.nextBtn = document.getElementById("next") as HTMLButtonElement;

    // Set up event listeners
    this.prevBtn.onclick = () => this.prev();
    this.nextBtn.onclick = () => this.next();
    this.commitBtn.onclick = () => this.commit();

    // Keyboard shortcuts
    document.addEventListener("keydown", (e) => {
      // Don't interfere when typing in contenteditable
      const activeEl = document.activeElement;
      if (activeEl instanceof HTMLElement && activeEl.contentEditable === "plaintext-only") {
        return;
      }

      // Focus editable fields by their hint key
      if (Object.values(KEYBINDS.focus).includes(e.key)) {
        const editable = this.transactionEl.querySelector(`[data-key="${e.key}"]`);
        if (editable instanceof HTMLElement) {
          editable.focus();
          e.preventDefault();
          return;
        }
      }

      if (KEYBINDS.prev.includes(e.key)) {
        void this.prev();
      } else if (KEYBINDS.next.includes(e.key)) {
        void this.next();
      } else if (e.key === KEYBINDS.commit) {
        if (!this.commitBtn.disabled) {
          void this.commit();
        }
      }
    });

    // Set up SSE listener for file changes
    this.setupFileChangeListener();
  }

  private setupFileChangeListener() {
    const eventSource = new EventSource("/api/file-changes");

    eventSource.onmessage = async () => {
      console.log("File change detected, reloading data...");
      await this.reloadData();
    };

    eventSource.onerror = (error) => {
      console.error("SSE connection error:", error);
      // Reconnection is handled automatically by EventSource
    };
  }

  async reloadData() {
    try {
      const data = await this.api.init();

      this.directives = data.items;
      this.availableAccounts = data.available_accounts;

      if (this.directives.length === 0) {
        this.showSuccess("No transactions to review!");
        this.transactionEl.textContent = "All done!";
        this.counterEl.textContent = "0/0";
        this.commitBtn.disabled = true;
        this.prevBtn.disabled = true;
        this.nextBtn.disabled = true;
        return;
      }

      // Adjust current index if it's now out of bounds
      if (this.currentIndex >= this.directives.length) {
        this.currentIndex = this.directives.length - 1;
      }

      await this.loadTransaction();
    } catch (err) {
      this.showError(`Failed to reload data: ${String(err)}`);
    }
  }

  async loadTransaction() {
    try {
      const currentDirective = this.directives[this.currentIndex];
      if (!currentDirective) {
        return;
      }

      const data = await this.api.getTransaction(currentDirective.id);

      // Initialize editState with default account if not present
      if (!this.editStates.has(currentDirective.id)) {
        this.editStates.set(currentDirective.id, {
          account: "Expenses:FIXME",
        });
      }

      // Reconstruct transaction as DOM with editable fields
      this.renderTransaction(data.transaction.transaction);

      this.counterEl.textContent = `Transaction ${this.currentIndex + 1}/${this.directives.length}`;

      this.clearMessage();
    } catch (err) {
      this.showError(`Failed to load transaction: ${String(err)}`);
    }
  }

  private renderTransaction(txn: import("./types").Transaction): void {
    this.transactionEl.innerHTML = "";

    const currentDirective = this.directives[this.currentIndex];
    const editState = currentDirective ? this.editStates.get(currentDirective.id) : undefined;

    // Helper to create contenteditable span
    const createEditable = (
      text: string,
      key: string,
      fieldName: "payee" | "narration" | "account",
    ): HTMLSpanElement => {
      const span = document.createElement("span");
      span.contentEditable = "plaintext-only";
      span.spellcheck = false;
      span.textContent = text;
      span.className = "editable";
      span.setAttribute("data-key", key);
      span.addEventListener("focus", () => this.selectAll(span));
      span.addEventListener("keydown", (e) => {
        if (e.key === "Escape" || e.key === "Enter") {
          e.preventDefault();
          e.stopPropagation();
          span.blur();
          window.getSelection()?.removeAllRanges();
        }
      });
      span.addEventListener("input", () => {
        const value = span.textContent?.trim() || "";
        if (currentDirective) {
          const state = this.editStates.get(currentDirective.id) || {
            account: "",
          };
          this.editStates.set(currentDirective.id, {
            ...state,
            [fieldName]: value,
          });
          this.updateCommitButton();
        }
      });
      return span;
    };

    // Helper to create colored span
    const createColored = (text: string, className: string): HTMLSpanElement => {
      const span = document.createElement("span");
      span.className = className;
      span.textContent = text;
      return span;
    };

    // First line: date flag payee narration
    this.transactionEl.appendChild(createColored(txn.date, "date"));
    this.transactionEl.appendChild(document.createTextNode(" " + txn.flag));

    if (txn.payee !== null) {
      this.transactionEl.appendChild(document.createTextNode(' "'));
      const payeeText = editState?.payee ?? txn.payee;
      this.transactionEl.appendChild(createEditable(payeeText, KEYBINDS.focus.payee, "payee"));
      this.transactionEl.appendChild(document.createTextNode('"'));
    }

    if (txn.narration !== null) {
      this.transactionEl.appendChild(document.createTextNode(' "'));
      const narrationText = editState?.narration ?? txn.narration;
      this.transactionEl.appendChild(
        createEditable(narrationText, KEYBINDS.focus.narration, "narration"),
      );
      this.transactionEl.appendChild(document.createTextNode('"'));
    }

    this.transactionEl.appendChild(document.createTextNode("\n"));

    // Tags and links
    if (txn.tags.length > 0) {
      this.transactionEl.appendChild(
        document.createTextNode("    " + txn.tags.map((t) => "#" + t).join(" ") + "\n"),
      );
    }
    if (txn.links.length > 0) {
      this.transactionEl.appendChild(
        document.createTextNode("    " + txn.links.map((l) => "^" + l).join(" ") + "\n"),
      );
    }

    // Postings
    for (const posting of txn.postings) {
      this.transactionEl.appendChild(document.createTextNode("    " + posting.account));
      if (posting.amount) {
        this.transactionEl.appendChild(document.createTextNode("  "));
        this.transactionEl.appendChild(createColored(posting.amount.value, "amount"));
        this.transactionEl.appendChild(document.createTextNode(" "));
        this.transactionEl.appendChild(createColored(posting.amount.currency, "currency"));
      }
      if (posting.cost) {
        this.transactionEl.appendChild(document.createTextNode(" " + posting.cost));
      }
      if (posting.price) {
        this.transactionEl.appendChild(document.createTextNode(" @ " + posting.price));
      }
      this.transactionEl.appendChild(document.createTextNode("\n"));
    }

    // Add editable expense account line
    this.transactionEl.appendChild(document.createTextNode("    "));
    const accountText = editState?.account || "";
    this.transactionEl.appendChild(createEditable(accountText, KEYBINDS.focus.account, "account"));
    this.transactionEl.appendChild(document.createTextNode("\n"));

    // Update commit button state
    this.updateCommitButton();
  }

  async commit() {
    const currentDirective = this.directives[this.currentIndex];
    if (!currentDirective) {
      return;
    }

    const editState = this.editStates.get(currentDirective.id);
    const expenseAccount = editState?.account;
    if (!expenseAccount || !expenseAccount.trim()) {
      this.showError("Please enter an expense account");
      return;
    }

    try {
      // Commit transaction with expense account
      const data = await this.api.commitTransaction(currentDirective.id, expenseAccount);

      if (data.remaining_count === 0) {
        this.showSuccess("All transactions committed!");
        this.transactionEl.textContent = "All done!";
        this.counterEl.textContent = "0/0";
        this.commitBtn.disabled = true;
        this.prevBtn.disabled = true;
        this.nextBtn.disabled = true;
        this.directives = [];
        return;
      }

      // Remove committed transaction's edit state and directive
      this.editStates.delete(currentDirective.id);
      this.directives.splice(this.currentIndex, 1);

      // Adjust index if needed
      if (this.currentIndex >= this.directives.length) {
        this.currentIndex = this.directives.length - 1;
      }

      await this.loadTransaction();
    } catch (err) {
      this.showError(`Failed to commit transaction: ${String(err)}`);
    }
  }

  async next() {
    if (this.directives.length === 0) {
      return;
    }
    this.currentIndex = (this.currentIndex + 1) % this.directives.length;
    await this.loadTransaction();
  }

  async prev() {
    if (this.directives.length === 0) {
      return;
    }
    this.currentIndex =
      this.currentIndex === 0 ? this.directives.length - 1 : this.currentIndex - 1;
    await this.loadTransaction();
  }

  private updateCommitButton() {
    const currentDirective = this.directives[this.currentIndex];
    if (!currentDirective) {
      this.commitBtn.disabled = true;
      return;
    }

    const editState = this.editStates.get(currentDirective.id);
    const hasAccount = editState?.account && editState.account.trim() !== "";
    this.commitBtn.disabled = !hasAccount;
  }

  private showError(message: string) {
    this.messageEl.className = "error";
    this.messageEl.textContent = message;
  }

  private showSuccess(message: string) {
    this.messageEl.className = "success";
    this.messageEl.textContent = message;
  }

  private clearMessage() {
    this.messageEl.className = "";
    this.messageEl.textContent = "";
  }

  private selectAll(element: HTMLElement) {
    const range = document.createRange();
    range.selectNodeContents(element);
    const selection = window.getSelection();
    if (selection) {
      selection.removeAllRanges();
      selection.addRange(range);
    }
  }
}

// Initialize app when DOM is ready
const app = new StagingApp();
void app.reloadData();
