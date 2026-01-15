interface Transaction {
  index: number;
  content: string;
}

interface TransactionResponse {
  transaction: Transaction;
  expense_account: string | null;
}

interface InitResponse {
  items: Transaction[];
  current_index: number;
}

interface CommitResponse {
  ok: boolean;
  remaining_count: number;
}

class StagingApp {
  private currentIndex = 0;
  private totalCount = 0;
  private currentAccount = "";

  private transactionEl: HTMLElement;
  private counterEl: HTMLElement;
  private accountInput: HTMLInputElement;
  private commitBtn: HTMLButtonElement;
  private messageEl: HTMLElement;

  constructor() {
    this.transactionEl = document.getElementById("transaction")!;
    this.counterEl = document.getElementById("counter")!;
    this.accountInput = document.getElementById("expense-account") as HTMLInputElement;
    this.commitBtn = document.getElementById("commit") as HTMLButtonElement;
    this.messageEl = document.getElementById("message")!;

    // Set up event listeners
    document.getElementById("prev")!.onclick = () => this.prev();
    document.getElementById("next")!.onclick = () => this.next();
    this.commitBtn.onclick = () => this.commit();

    // Auto-save account on input change
    this.accountInput.oninput = () => {
      this.currentAccount = this.accountInput.value;
      this.commitBtn.disabled = this.accountInput.value.trim() === "";
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
  }

  async init() {
    try {
      const resp = await fetch("/api/init");
      if (!resp.ok) {
        throw new Error(`Failed to initialize: ${resp.statusText}`);
      }

      const data: InitResponse = await resp.json();

      if (data.items.length === 0) {
        this.showSuccess("No transactions to review!");
        this.transactionEl.textContent = "All done!";
        this.counterEl.textContent = "0/0";
        return;
      }

      this.totalCount = data.items.length;
      this.currentIndex = 0;
      await this.loadTransaction();
    } catch (err) {
      this.showError(`Failed to load transactions: ${err}`);
    }
  }

  async loadTransaction() {
    try {
      // Save current account before loading new transaction
      if (this.currentAccount) {
        await this.saveAccount();
      }

      const resp = await fetch(`/api/transaction/${this.currentIndex}`);
      if (!resp.ok) {
        throw new Error(`Failed to load transaction: ${resp.statusText}`);
      }

      const data: TransactionResponse = await resp.json();

      this.transactionEl.textContent = data.transaction.content;
      this.counterEl.textContent = `Transaction ${this.currentIndex + 1}/${this.totalCount}`;

      this.accountInput.value = data.expense_account || "";
      this.currentAccount = this.accountInput.value;
      this.commitBtn.disabled = this.accountInput.value.trim() === "";

      this.clearMessage();
    } catch (err) {
      this.showError(`Failed to load transaction: ${err}`);
    }
  }

  async saveAccount() {
    if (!this.currentAccount.trim()) {
      return;
    }

    try {
      const resp = await fetch(`/api/transaction/${this.currentIndex}/account`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ expense_account: this.currentAccount }),
      });

      if (!resp.ok) {
        throw new Error(`Failed to save account: ${resp.statusText}`);
      }
    } catch (err) {
      this.showError(`Failed to save account: ${err}`);
      throw err;
    }
  }

  async commit() {
    if (!this.currentAccount.trim()) {
      this.showError("Please enter an expense account");
      return;
    }

    try {
      // Save account first
      await this.saveAccount();

      // Commit transaction
      const resp = await fetch(`/api/transaction/${this.currentIndex}/commit`, {
        method: "POST",
      });

      if (!resp.ok) {
        const errorData = await resp.json().catch(() => null);
        const errorMsg = errorData?.error ?? resp.statusText;
        throw new Error(errorMsg);
      }

      const data: CommitResponse = await resp.json();

      if (data.remaining_count === 0) {
        this.showSuccess("All transactions committed!");
        this.transactionEl.textContent = "All done!";
        this.counterEl.textContent = "0/0";
        this.accountInput.disabled = true;
        this.commitBtn.disabled = true;
        return;
      }

      this.showSuccess("Transaction committed!");
      this.totalCount = data.remaining_count;

      // Adjust index if needed
      if (this.currentIndex >= this.totalCount) {
        this.currentIndex = this.totalCount - 1;
      }

      await this.loadTransaction();
    } catch (err) {
      this.showError(`Failed to commit transaction: ${err}`);
    }
  }

  async next() {
    this.currentIndex = (this.currentIndex + 1) % this.totalCount;
    await this.loadTransaction();
  }

  async prev() {
    this.currentIndex = this.currentIndex === 0 ? this.totalCount - 1 : this.currentIndex - 1;
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
}

// Initialize app when DOM is ready
const app = new StagingApp();
app.init();
