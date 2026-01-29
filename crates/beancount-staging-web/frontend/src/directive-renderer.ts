import type { Transaction, Balance } from "./model/beancount";
import type { TransactionPatch } from "./api";
import { Autocomplete, type FilterFunction } from "./autocomplete";

export type EditState = TransactionPatch;

const EDITABLE_SHORTCUTS = {
  payee: "p",
  narration: "n",
  account: "a",
};

export class DirectiveRenderer {
  private autocomplete: Autocomplete;

  constructor(
    private container: HTMLElement,
    private onInput: (field: "payee" | "narration" | "account", value: string) => void,
    availableAccounts: string[],
    filterFn: FilterFunction,
  ) {
    // Set up autocomplete
    this.autocomplete = new Autocomplete(availableAccounts, () => {}, filterFn);

    // Set up global keyboard handler for focusing fields
    document.addEventListener("keydown", (e) => this.handleFocusShortcuts(e));
  }

  setAvailableAccounts(accounts: string[]) {
    this.autocomplete.setAvailableItems(accounts);
  }

  render(txn: Transaction, editState?: EditState): void {
    this.container.innerHTML = "";

    // First line: date flag payee narration
    this.container.appendChild(this.createColored(txn.date, "date"));
    this.container.appendChild(document.createTextNode(" " + txn.flag));

    if (txn.payee !== null) {
      this.container.appendChild(document.createTextNode(' "'));
      const payeeText = editState?.payee ?? txn.payee;
      this.container.appendChild(
        this.createTextField(payeeText, EDITABLE_SHORTCUTS.payee, "payee"),
      );
      this.container.appendChild(document.createTextNode('"'));
    }

    if (txn.narration !== null) {
      this.container.appendChild(document.createTextNode(' "'));
      const narrationText = editState?.narration ?? txn.narration;
      this.container.appendChild(
        this.createTextField(narrationText, EDITABLE_SHORTCUTS.narration, "narration"),
      );
      this.container.appendChild(document.createTextNode('"'));
    }

    this.container.appendChild(document.createTextNode("\n"));

    // Tags and links
    if (txn.tags.length > 0) {
      this.container.appendChild(
        document.createTextNode("    " + txn.tags.map((t) => "#" + t).join(" ") + "\n"),
      );
    }
    if (txn.links.length > 0) {
      this.container.appendChild(
        document.createTextNode("    " + txn.links.map((l) => "^" + l).join(" ") + "\n"),
      );
    }

    // Check if transaction is already balanced (all postings have amounts AND sum to zero)
    const isBalanced =
      txn.postings.length >= 2 &&
      txn.postings.every((p) => p.amount !== null) &&
      this.isTransactionBalanced(txn);

    // Find if transaction already has an expense posting (posting without amount)
    const expensePosting = txn.postings.find((p) => !p.amount);
    let hasEditableLine = false;

    // Postings
    for (const posting of txn.postings) {
      this.container.appendChild(document.createTextNode("    "));

      // If this is the expense posting without amount, make it editable
      if (posting === expensePosting) {
        const accountText = editState?.account ?? posting.account;
        this.container.appendChild(
          this.createAccountField(accountText, EDITABLE_SHORTCUTS.account),
        );
        hasEditableLine = true;
      } else {
        this.container.appendChild(document.createTextNode(posting.account));
      }

      if (posting.amount) {
        this.container.appendChild(document.createTextNode("  "));
        this.container.appendChild(this.createColored(posting.amount.value, "amount"));
        this.container.appendChild(document.createTextNode(" "));
        this.container.appendChild(this.createColored(posting.amount.currency, "currency"));
      }
      if (posting.cost) {
        this.container.appendChild(document.createTextNode(" " + posting.cost));
      }
      if (posting.price) {
        this.container.appendChild(document.createTextNode(" @ " + posting.price));
      }
      this.container.appendChild(document.createTextNode("\n"));
    }

    // Only add editable expense account line if:
    // 1. Transaction is not already balanced
    // 2. There wasn't already an editable posting
    if (!isBalanced && !hasEditableLine) {
      this.container.appendChild(document.createTextNode("    "));
      const accountText = editState?.account ?? "";
      this.container.appendChild(this.createAccountField(accountText, EDITABLE_SHORTCUTS.account));
      this.container.appendChild(document.createTextNode("\n"));
    }
  }

  private createTextField(
    text: string,
    key: string,
    fieldName: "payee" | "narration",
  ): HTMLSpanElement {
    const span = document.createElement("span");
    span.contentEditable = "plaintext-only";
    span.spellcheck = false;
    span.textContent = text;
    span.className = "editable";
    span.setAttribute("data-key", key);

    span.addEventListener("focus", () => this.selectAll(span));
    span.addEventListener("keydown", (e) => {
      // Stop all keyboard events from bubbling to prevent global shortcuts
      e.stopPropagation();

      if (e.key === "Escape" || e.key === "Enter") {
        e.preventDefault();
        span.blur();
        window.getSelection()?.removeAllRanges();
      }
    });
    span.addEventListener("input", () => {
      const value = span.textContent?.trim() || "";
      this.onInput(fieldName, value);
    });

    return span;
  }

  private createAccountField(text: string, key: string): HTMLSpanElement {
    const span = document.createElement("span");
    span.contentEditable = "plaintext-only";
    span.spellcheck = false;
    span.textContent = text;
    span.className = "editable";
    span.setAttribute("data-key", key);

    span.addEventListener("focus", () => {
      this.selectAll(span);
      this.autocomplete.show(span);
    });

    span.addEventListener("input", () => {
      const value = span.textContent?.trim() || "";
      this.onInput("account", value);
      this.autocomplete.show(span);
    });

    span.addEventListener("keydown", (e) => {
      // Stop all keyboard events from bubbling to prevent global shortcuts
      e.stopPropagation();

      // Handle autocomplete navigation
      if (this.autocomplete.isVisible()) {
        if (e.key === "ArrowDown") {
          e.preventDefault();
          this.autocomplete.updateSelection("down");
          return;
        } else if (e.key === "ArrowUp") {
          e.preventDefault();
          this.autocomplete.updateSelection("up");
          return;
        } else if (e.key === "Enter" || e.key == "Tab") {
          e.preventDefault();
          this.autocomplete.selectCurrent();
          window.getSelection()?.removeAllRanges();
          return;
        } else if (e.key === "Escape") {
          e.preventDefault();
          this.autocomplete.hide();
          // Fall through to blur
        }
      }

      // Default escape/enter behavior
      if (e.key === "Escape" || e.key === "Enter") {
        e.preventDefault();
        span.blur();
        window.getSelection()?.removeAllRanges();
      }
    });

    span.addEventListener("blur", () => {
      // Delay to allow click events to fire
      setTimeout(() => this.autocomplete.hide(), 200);
    });

    return span;
  }

  private createColored(text: string, className: string): HTMLSpanElement {
    const span = document.createElement("span");
    span.className = className;
    span.textContent = text;
    return span;
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

  renderBalance(bal: Balance): void {
    this.container.innerHTML = "";

    // Format: date balance account amount [tolerance]
    this.container.appendChild(this.createColored(bal.date, "date"));
    this.container.appendChild(document.createTextNode(" balance "));
    this.container.appendChild(document.createTextNode(bal.account));
    this.container.appendChild(document.createTextNode("  "));
    this.container.appendChild(this.createColored(bal.amount.value, "amount"));
    this.container.appendChild(document.createTextNode(" "));
    this.container.appendChild(this.createColored(bal.amount.currency, "currency"));

    if (bal.tolerance) {
      this.container.appendChild(document.createTextNode(" ~ "));
      this.container.appendChild(document.createTextNode(bal.tolerance));
    }

    this.container.appendChild(document.createTextNode("\n"));
  }

  private handleFocusShortcuts(e: KeyboardEvent) {
    // Focus editable fields by their hint key
    if (Object.values(EDITABLE_SHORTCUTS).includes(e.key)) {
      const editable = this.container.querySelector(`[data-key="${e.key}"]`);
      if (editable instanceof HTMLElement) {
        editable.focus();
        e.preventDefault();
      }
    }
  }

  private isTransactionBalanced(txn: Transaction): boolean {
    // Group amounts by currency
    const totalsByCurrency = new Map<string, number>();

    for (const posting of txn.postings) {
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
