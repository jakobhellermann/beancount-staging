import { describe, it, expect, beforeEach, vi, type Mock } from "vitest";
import { Autocomplete } from "../src/autocomplete";

describe("Autocomplete", () => {
  let autocomplete: Autocomplete;
  let onSelectMock: Mock<(item: string) => void>;
  let filterMock: Mock<(query: string, items: string[]) => string[]>;
  const testAccounts = ["Expenses:Food", "Expenses:Travel", "Assets:Bank"];

  beforeEach(() => {
    document.body.innerHTML = "";
    onSelectMock = vi.fn();
    filterMock = vi.fn((query: string, items: string[]) => {
      return items.filter((item: string) => item.toLowerCase().includes(query.toLowerCase()));
    });
    autocomplete = new Autocomplete(testAccounts, onSelectMock, filterMock);
  });

  describe("initialization", () => {
    it("should create dropdown element", () => {
      const dropdown = document.querySelector(".autocomplete-dropdown");
      expect(dropdown).toBeTruthy();
      expect(dropdown?.getAttribute("style")).toContain("display: none");
    });

    it("should not be visible initially", () => {
      expect(autocomplete.isVisible()).toBe(false);
    });
  });

  describe("setAvailableItems", () => {
    it("should update available items", () => {
      const newItems = ["Income:Salary", "Expenses:Rent"];
      autocomplete.setAvailableItems(newItems);

      const input = document.createElement("span");
      input.textContent = "income";
      document.body.appendChild(input);

      autocomplete.show(input);
      expect(filterMock).toHaveBeenCalledWith("income", newItems);
    });
  });

  describe("show", () => {
    it("should display dropdown with filtered items", () => {
      const input = document.createElement("span");
      input.textContent = "expense";
      document.body.appendChild(input);

      autocomplete.show(input);

      expect(autocomplete.isVisible()).toBe(true);
      expect(filterMock).toHaveBeenCalledWith("expense", testAccounts);

      const dropdown = document.querySelector(".autocomplete-dropdown") as HTMLElement;
      expect(dropdown?.style.display).toBe("block");

      const items = dropdown?.querySelectorAll(".autocomplete-item");
      expect(items?.length).toBe(2); // Expenses:Food, Expenses:Travel
    });

    it("should hide dropdown when no items match", () => {
      const input = document.createElement("span");
      input.textContent = "nomatch";
      document.body.appendChild(input);

      filterMock.mockReturnValue([]);
      autocomplete.show(input);

      expect(autocomplete.isVisible()).toBe(false);
    });

    it("should handle empty query", () => {
      const input = document.createElement("span");
      input.textContent = "";
      document.body.appendChild(input);

      autocomplete.show(input);

      expect(filterMock).toHaveBeenCalledWith("", testAccounts);
    });

    it("should position dropdown below input when space available", () => {
      const input = document.createElement("span");
      input.textContent = "expense";
      document.body.appendChild(input);

      // Mock getBoundingClientRect to simulate position at top of viewport
      vi.spyOn(input, "getBoundingClientRect").mockReturnValue({
        top: 100,
        bottom: 120,
        left: 50,
        right: 200,
        width: 150,
        height: 20,
        x: 50,
        y: 100,
        toJSON: () => ({}),
      });

      autocomplete.show(input);

      const dropdown = document.querySelector(".autocomplete-dropdown") as HTMLElement;
      expect(dropdown?.style.top).toBeTruthy();
      expect(dropdown?.style.bottom).toBe("auto");
    });
  });

  describe("hide", () => {
    it("should hide dropdown and reset state", () => {
      const input = document.createElement("span");
      input.textContent = "expense";
      document.body.appendChild(input);

      autocomplete.show(input);
      expect(autocomplete.isVisible()).toBe(true);

      autocomplete.hide();
      expect(autocomplete.isVisible()).toBe(false);

      const dropdown = document.querySelector(".autocomplete-dropdown") as HTMLElement;
      expect(dropdown?.style.display).toBe("none");
    });
  });

  describe("updateSelection", () => {
    it("should cycle through items with down direction", () => {
      const input = document.createElement("span");
      input.textContent = "expense";
      document.body.appendChild(input);

      autocomplete.show(input);

      autocomplete.updateSelection("down");
      let selected = document.querySelector(".autocomplete-item.selected");
      expect(selected?.textContent).toBe("Expenses:Food");

      autocomplete.updateSelection("down");
      selected = document.querySelector(".autocomplete-item.selected");
      expect(selected?.textContent).toBe("Expenses:Travel");

      // Should wrap around
      autocomplete.updateSelection("down");
      selected = document.querySelector(".autocomplete-item.selected");
      expect(selected?.textContent).toBe("Expenses:Food");
    });

    it("should cycle through items with up direction", () => {
      const input = document.createElement("span");
      input.textContent = "expense";
      document.body.appendChild(input);

      autocomplete.show(input);

      autocomplete.updateSelection("up");
      let selected = document.querySelector(".autocomplete-item.selected");
      expect(selected?.textContent).toBe("Expenses:Travel");

      autocomplete.updateSelection("up");
      selected = document.querySelector(".autocomplete-item.selected");
      expect(selected?.textContent).toBe("Expenses:Food");
    });

    it("should do nothing when not visible", () => {
      autocomplete.updateSelection("down");
      expect(autocomplete.isVisible()).toBe(false);
    });
  });

  describe("selectCurrent", () => {
    it("should select highlighted item", () => {
      const input = document.createElement("span");
      input.textContent = "expense";
      input.blur = vi.fn(); // Mock blur
      document.body.appendChild(input);

      autocomplete.show(input);
      autocomplete.updateSelection("down");
      autocomplete.selectCurrent();

      expect(input.textContent).toBe("Expenses:Food");
      expect(autocomplete.isVisible()).toBe(false);
      expect(onSelectMock).toHaveBeenCalledWith("Expenses:Food");
    });

    it("should select first item when nothing highlighted", () => {
      const input = document.createElement("span");
      input.textContent = "expense";
      input.blur = vi.fn(); // Mock blur
      document.body.appendChild(input);

      autocomplete.show(input);
      autocomplete.selectCurrent();

      expect(input.textContent).toBe("Expenses:Food");
      expect(onSelectMock).toHaveBeenCalledWith("Expenses:Food");
    });

    it("should hide and blur when no items available", () => {
      const input = document.createElement("span");
      input.textContent = "expense";
      input.blur = vi.fn(); // Mock blur
      document.body.appendChild(input);

      const blurSpy = vi.spyOn(input, "blur");

      // First show with items
      filterMock.mockReturnValue(["Expenses:Food"]);
      autocomplete.show(input);

      // Then manually clear items to test edge case
      (autocomplete as any).items = [];

      autocomplete.selectCurrent();

      expect(autocomplete.isVisible()).toBe(false);
      expect(blurSpy).toHaveBeenCalled();
    });
  });

  describe("item interaction", () => {
    it("should select item on click", () => {
      const input = document.createElement("span");
      input.textContent = "expense";
      input.blur = vi.fn(); // Mock blur
      document.body.appendChild(input);

      autocomplete.show(input);

      const items = document.querySelectorAll(".autocomplete-item");
      (items[1] as HTMLElement).click();

      expect(input.textContent).toBe("Expenses:Travel");
      expect(onSelectMock).toHaveBeenCalledWith("Expenses:Travel");
      expect(autocomplete.isVisible()).toBe(false);
    });

    it("should highlight item on mouseenter", () => {
      const input = document.createElement("span");
      input.textContent = "expense";
      document.body.appendChild(input);

      autocomplete.show(input);

      const items = document.querySelectorAll(".autocomplete-item");
      const event = new MouseEvent("mouseenter", { bubbles: true });
      items[1].dispatchEvent(event);

      expect(items[1].classList.contains("selected")).toBe(true);
      expect(items[0].classList.contains("selected")).toBe(false);
    });
  });

  describe("click outside", () => {
    it("should hide dropdown when clicking outside", () => {
      const input = document.createElement("span");
      input.textContent = "expense";
      document.body.appendChild(input);

      autocomplete.show(input);
      expect(autocomplete.isVisible()).toBe(true);

      const outsideElement = document.createElement("div");
      document.body.appendChild(outsideElement);

      const clickEvent = new MouseEvent("click", { bubbles: true });
      Object.defineProperty(clickEvent, "target", {
        value: outsideElement,
        enumerable: true,
      });
      document.dispatchEvent(clickEvent);

      expect(autocomplete.isVisible()).toBe(false);
    });

    it("should not hide when clicking on input", () => {
      const input = document.createElement("span");
      input.textContent = "expense";
      document.body.appendChild(input);

      autocomplete.show(input);

      const clickEvent = new MouseEvent("click", { bubbles: true });
      Object.defineProperty(clickEvent, "target", { value: input, enumerable: true });
      document.dispatchEvent(clickEvent);

      expect(autocomplete.isVisible()).toBe(true);
    });
  });
});
