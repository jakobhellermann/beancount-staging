import { ApiClient } from "./api";

interface EditState {
  account: string;
}

class StagingApp {
  private api = new ApiClient();
  private directives: import("./types").Directive[] = [];
  private currentIndex = 0;
  private availableAccounts: string[] = [];
  private editStates: Map<string, EditState> = new Map();

  private transactionEl: HTMLElement;
  private counterEl: HTMLElement;
  private accountInput: HTMLInputElement;
  private accountDatalist: HTMLDataListElement;
  private commitBtn: HTMLButtonElement;
  private messageEl: HTMLElement;
  private prevBtn: HTMLButtonElement;
  private nextBtn: HTMLButtonElement;

  constructor() {
    this.transactionEl = document.getElementById("transaction")!;
    this.counterEl = document.getElementById("counter")!;
    this.accountInput = document.getElementById("expense-account") as HTMLInputElement;
    this.accountDatalist = document.getElementById("account-list") as HTMLDataListElement;
    this.commitBtn = document.getElementById("commit") as HTMLButtonElement;
    this.messageEl = document.getElementById("message")!;
    this.prevBtn = document.getElementById("prev") as HTMLButtonElement;
    this.nextBtn = document.getElementById("next") as HTMLButtonElement;

    // Set up event listeners
    this.prevBtn.onclick = () => this.prev();
    this.nextBtn.onclick = () => this.next();
    this.commitBtn.onclick = () => this.commit();

    // Update edit state on input change
    this.accountInput.oninput = () => {
      const value = this.accountInput.value.trim();
      const currentDirective = this.directives[this.currentIndex];
      if (currentDirective) {
        if (value) {
          this.editStates.set(currentDirective.id, { account: value });
        } else {
          this.editStates.delete(currentDirective.id);
        }
      }
      this.commitBtn.disabled = value === "";
    };

    // Keyboard shortcuts
    document.addEventListener("keydown", (e) => {
      // Don't interfere when typing in input
      if (document.activeElement === this.accountInput) {
        return;
      }

      if (e.key === "ArrowLeft" || e.key === "h") {
        this.prev();
      } else if (e.key === "ArrowRight" || e.key === "l") {
        this.next();
      } else if (e.key === "Enter") {
        if (!this.commitBtn.disabled) {
          this.commit();
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
      this.populateAccountList();

      if (this.directives.length === 0) {
        this.showSuccess("No transactions to review!");
        this.transactionEl.textContent = "All done!";
        this.counterEl.textContent = "0/0";
        this.accountInput.disabled = true;
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
      this.showError(`Failed to reload data: ${err}`);
    }
  }

  async loadTransaction() {
    try {
      const currentDirective = this.directives[this.currentIndex];
      if (!currentDirective) {
        return;
      }

      const data = await this.api.getTransaction(currentDirective.id);

      // Reconstruct transaction text from structured data
      this.transactionEl.textContent = this.formatTransaction(data.transaction.transaction);

      this.counterEl.textContent = `Transaction ${this.currentIndex + 1}/${this.directives.length}`;

      // Load account from edit state
      const editState = this.editStates.get(currentDirective.id);
      const savedAccount = editState?.account || "";
      this.accountInput.value = savedAccount;
      this.commitBtn.disabled = savedAccount.trim() === "";

      this.clearMessage();
    } catch (err) {
      this.showError(`Failed to load transaction: ${err}`);
    }
  }

  private formatTransaction(txn: import("./types").Transaction): string {
    const lines: string[] = [];

    // First line: date flag payee narration
    let firstLine = txn.date;
    firstLine += " " + txn.flag;
    if (txn.payee) {
      firstLine += ' "' + txn.payee + '"';
    }
    if (txn.narration) {
      firstLine += ' "' + txn.narration + '"';
    }
    lines.push(firstLine);

    // Tags and links
    if (txn.tags.length > 0) {
      lines.push("    " + txn.tags.map((t) => "#" + t).join(" "));
    }
    if (txn.links.length > 0) {
      lines.push("    " + txn.links.map((l) => "^" + l).join(" "));
    }

    // Postings
    for (const posting of txn.postings) {
      let postingLine = "    " + posting.account;
      if (posting.amount) {
        postingLine += "  " + posting.amount.value + " " + posting.amount.currency;
      }
      if (posting.cost) {
        postingLine += " " + posting.cost;
      }
      if (posting.price) {
        postingLine += " @ " + posting.price;
      }
      lines.push(postingLine);
    }

    return lines.join("\n");
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
        this.accountInput.disabled = true;
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
      this.showError(`Failed to commit transaction: ${err}`);
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

  private populateAccountList() {
    this.accountDatalist.replaceChildren();
    for (const account of this.availableAccounts) {
      const option = document.createElement("option");
      option.value = account;
      this.accountDatalist.appendChild(option);
    }
  }
}

// Initialize app when DOM is ready
const app = new StagingApp();
app.reloadData();
