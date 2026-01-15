import { ApiClient } from "./api";

class StagingApp {
  private api = new ApiClient();
  private currentIndex = 0;
  private totalCount = 0;
  private currentAccount = "";

  private transactionEl: HTMLElement;
  private counterEl: HTMLElement;
  private accountInput: HTMLInputElement;
  private commitBtn: HTMLButtonElement;
  private messageEl: HTMLElement;
  private prevBtn: HTMLButtonElement;
  private nextBtn: HTMLButtonElement;

  constructor() {
    this.transactionEl = document.getElementById("transaction")!;
    this.counterEl = document.getElementById("counter")!;
    this.accountInput = document.getElementById("expense-account") as HTMLInputElement;
    this.commitBtn = document.getElementById("commit") as HTMLButtonElement;
    this.messageEl = document.getElementById("message")!;
    this.prevBtn = document.getElementById("prev") as HTMLButtonElement;
    this.nextBtn = document.getElementById("next") as HTMLButtonElement;

    // Set up event listeners
    this.prevBtn.onclick = () => this.prev();
    this.nextBtn.onclick = () => this.next();
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
      const data = await this.api.init();

      if (data.items.length === 0) {
        this.showSuccess("No transactions to review!");
        this.transactionEl.textContent = "All done!";
        this.counterEl.textContent = "0/0";
        this.accountInput.disabled = true;
        this.commitBtn.disabled = true;
        this.prevBtn.disabled = true;
        this.nextBtn.disabled = true;
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

      const data = await this.api.getTransaction(this.currentIndex);

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
      await this.api.saveAccount(this.currentIndex, this.currentAccount);
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
      const data = await this.api.commitTransaction(this.currentIndex);

      if (data.remaining_count === 0) {
        this.showSuccess("All transactions committed!");
        this.transactionEl.textContent = "All done!";
        this.counterEl.textContent = "0/0";
        this.accountInput.disabled = true;
        this.commitBtn.disabled = true;
        this.prevBtn.disabled = true;
        this.nextBtn.disabled = true;
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
    if (this.totalCount === 0) {
      return;
    }
    this.currentIndex = (this.currentIndex + 1) % this.totalCount;
    await this.loadTransaction();
  }

  async prev() {
    if (this.totalCount === 0) {
      return;
    }
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
