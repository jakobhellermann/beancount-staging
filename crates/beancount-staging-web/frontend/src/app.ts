import { ApiClient, type TransactionPatch } from "./api";
import { DirectiveRenderer, type EditState } from "./directive-renderer";
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

  private renderer: DirectiveRenderer;

  constructor() {
    this.transactionEl = document.getElementById("transaction")!;
    this.counterEl = document.getElementById("counter")!;
    this.commitBtn = document.getElementById("commit") as HTMLButtonElement;
    this.messageEl = document.getElementById("message")!;
    this.prevBtn = document.getElementById("prev") as HTMLButtonElement;
    this.nextBtn = document.getElementById("next") as HTMLButtonElement;

    // Initialize renderer
    this.renderer = new DirectiveRenderer(
      this.transactionEl,
      (field, value) => {
        const currentDirective = this.directives[this.currentIndex];
        if (currentDirective) {
          const state = this.editStates.get(currentDirective.id) ?? { account: "" };
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

      // Check if transaction is already balanced
      const isBalanced = this.isTransactionBalanced(data.transaction);

      // Initialize editState with predicted or default account if not present
      if (!this.editStates.has(currentDirective.id)) {
        // For balanced transactions, don't set an account
        if (isBalanced) {
          this.editStates.set(currentDirective.id, {});
        } else {
          // Check if transaction already has an expense posting (posting without amount)
          let defaultAccount = data.predicted_account ?? "Expenses:";
          if (data.transaction.type === "transaction") {
            const expensePosting = data.transaction.postings.find((p) => !p.amount);
            if (expensePosting) {
              defaultAccount = expensePosting.account;
            }
          }

          this.editStates.set(currentDirective.id, {
            account: defaultAccount,
          });
        }
      }

      const editState = this.editStates.get(currentDirective.id);

      // Render directive based on type
      if (data.transaction.type === "transaction") {
        this.renderer.render(data.transaction, editState);
        this.counterEl.textContent = `Transaction ${this.currentIndex + 1}/${this.directives.length}`;
      } else if (data.transaction.type === "balance") {
        this.renderer.renderBalance(data.transaction);
        this.counterEl.textContent = `Balance ${this.currentIndex + 1}/${this.directives.length}`;
      }

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

    // Check if transaction is balanced
    const isBalanced = this.isTransactionBalanced(currentDirective);

    const editState = this.editStates.get(currentDirective.id);

    // For unbalanced transactions, require an account
    if (!isBalanced && (!editState?.account || !editState.account.trim())) {
      this.showError("Please enter an expense account");
      return;
    }

    try {
      // Commit transaction with patch containing all edited fields
      // For balanced transactions, omit the account field
      const patch: TransactionPatch = {};
      if (editState?.payee) {
        patch.payee = editState.payee;
      }
      if (editState?.narration) {
        patch.narration = editState.narration;
      }
      if (!isBalanced && editState?.account) {
        patch.account = editState.account;
      }

      const data = await this.api.commitTransaction(currentDirective.id, patch);

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

    // Check if transaction is balanced
    if (this.isTransactionBalanced(currentDirective)) {
      // Balanced transactions can always be committed
      this.commitBtn.disabled = false;
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

  private isTransactionBalanced(directive: Directive): boolean {
    if (directive.type !== "transaction") {
      return false;
    }

    // All postings must have amounts
    if (!directive.postings.every((p) => p.amount !== null)) {
      return false;
    }

    // Group amounts by currency
    const totalsByCurrency = new Map<string, number>();

    for (const posting of directive.postings) {
      if (!posting.amount) {
        return false;
      }

      const currency = posting.amount.currency;
      const value = parseFloat(posting.amount.value);

      const current = totalsByCurrency.get(currency) ?? 0;
      totalsByCurrency.set(currency, current + value);
    }

    // Check if all currency totals are zero (within floating point precision)
    for (const total of totalsByCurrency.values()) {
      if (Math.abs(total) > 0.005) {
        return false;
      }
    }

    return true;
  }
}

const app = new StagingApp();
void app.reloadData();
