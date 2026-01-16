import type { Transaction } from "./model/beancount";
import { Autocomplete, type FilterFunction } from "./autocomplete";

export interface EditState {
  account: string;
  payee?: string;
  narration?: string;
}

const EDITABLE_SHORTCUTS = {
  payee: "p",
  narration: "n",
  account: "a",
};

export class TransactionRenderer {
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

    // Postings
    for (const posting of txn.postings) {
      this.container.appendChild(document.createTextNode("    " + posting.account));
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

    // Add editable expense account line
    this.container.appendChild(document.createTextNode("    "));
    const accountText = editState?.account ?? "";
    this.container.appendChild(this.createAccountField(accountText, EDITABLE_SHORTCUTS.account));
    this.container.appendChild(document.createTextNode("\n"));
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
        if (e.key === "ArrowDown" || (e.key === "Tab" && !e.shiftKey)) {
          e.preventDefault();
          this.autocomplete.updateSelection("down");
          return;
        } else if (e.key === "ArrowUp" || (e.key === "Tab" && e.shiftKey)) {
          e.preventDefault();
          this.autocomplete.updateSelection("up");
          return;
        } else if (e.key === "Enter") {
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
}
