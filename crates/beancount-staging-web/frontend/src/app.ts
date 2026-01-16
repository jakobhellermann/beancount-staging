import { ApiClient } from "./api";
import { TransactionRenderer, type EditState } from "./transaction-renderer";
import { filterAccounts } from "./account-filter";
import type { Directive } from "./model/beancount";

class StagingApp {
  private api = new ApiClient();
  private directives: Directive[] = [];
  private currentIndex = 0;
  private editStates: Map<string, EditState> = new Map();

  private transactionEl: HTMLElement;
  private counterEl: HTMLElement;
  private commitBtn: HTMLButtonElement;
  private messageEl: HTMLElement;
  private prevBtn: HTMLButtonElement;
  private nextBtn: HTMLButtonElement;

  private renderer: TransactionRenderer;

  constructor() {
    this.transactionEl = document.getElementById("transaction")!;
    this.counterEl = document.getElementById("counter")!;
    this.commitBtn = document.getElementById("commit") as HTMLButtonElement;
    this.messageEl = document.getElementById("message")!;
    this.prevBtn = document.getElementById("prev") as HTMLButtonElement;
    this.nextBtn = document.getElementById("next") as HTMLButtonElement;

    // Initialize renderer
    this.renderer = new TransactionRenderer(
      this.transactionEl,
      (field, value) => {
        const currentDirective = this.directives[this.currentIndex];
        if (currentDirective) {
          const state = this.editStates.get(currentDirective.id) || { account: "" };
          this.editStates.set(currentDirective.id, { ...state, [field]: value });
          this.updateCommitButton();
        }
      },
      [],
      filterAccounts,
    );

    // Set up button event listeners
    this.prevBtn.onclick = () => this.prev();
    this.nextBtn.onclick = () => this.next();
    this.commitBtn.onclick = () => this.commit();

    // Set up keyboard shortcuts
    document.addEventListener("keydown", (e) => this.handleKeyboardShortcuts(e));

    // Set up SSE listener for file changes
    this.setupFileChangeListener();
  }

  private handleKeyboardShortcuts(e: KeyboardEvent) {
    const KEYBINDS = {
      prev: ["ArrowLeft", "h"],
      next: ["ArrowRight", "l"],
      commit: "Enter",
    };

    if (KEYBINDS.prev.includes(e.key)) {
      void this.prev();
    } else if (KEYBINDS.next.includes(e.key)) {
      void this.next();
    } else if (e.key === KEYBINDS.commit) {
      if (!this.commitBtn.disabled) {
        void this.commit();
      }
    }
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
      this.renderer.setAvailableAccounts(data.available_accounts);

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
          account: "Expenses:",
        });
      }

      const editState = this.editStates.get(currentDirective.id);

      // Render transaction
      this.renderer.render(data.transaction.transaction, editState);

      this.counterEl.textContent = `Transaction ${this.currentIndex + 1}/${this.directives.length}`;

      this.clearMessage();
      this.updateCommitButton();
    } catch (err) {
      this.showError(`Failed to load transaction: ${String(err)}`);
    }
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
}

const app = new StagingApp();
void app.reloadData();
