import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { describe, expect, it } from "vitest";

import { Input } from "./input";

interface FormValues {
  endpoint: string;
}

function RegisterHarness({ defaults }: { defaults: FormValues }) {
  const { register, reset } = useForm<FormValues>({ defaultValues: defaults });
  return (
    <form>
      <Input data-testid="endpoint" {...register("endpoint")} />
      <button
        type="button"
        onClick={() => reset({ endpoint: "43.240.223.138/u3400" })}
      >
        恢复默认
      </button>
    </form>
  );
}

describe("Input", () => {
  it("ref 转发到底层 input 元素（react-hook-form register 依赖）", () => {
    let node: HTMLInputElement | null = null;
    render(<Input ref={(el) => (node = el)} data-testid="plain" />);
    expect(node).toBeInstanceOf(HTMLInputElement);
    expect(node).toBe(screen.getByTestId("plain"));
  });

  it("受控值渲染进 input 并随输入更新", () => {
    function Controlled() {
      const [value, setValue] = useState("192.168.1.1/u3400");
      return (
        <Input
          data-testid="controlled"
          value={value}
          onChange={(event) => setValue(event.target.value)}
        />
      );
    }
    render(<Controlled />);
    const input = screen.getByTestId("controlled") as HTMLInputElement;
    expect(input.value).toBe("192.168.1.1/u3400");
    fireEvent.change(input, { target: { value: "10.0.0.1/u3401" } });
    expect(input.value).toBe("10.0.0.1/u3401");
  });

  it("register 受控回显：挂载默认值与 reset 写入均落到 DOM", () => {
    render(<RegisterHarness defaults={{ endpoint: "10.0.0.9/u3400" }} />);
    const input = screen.getByTestId("endpoint") as HTMLInputElement;
    expect(input.value).toBe("10.0.0.9/u3400");
    fireEvent.click(screen.getByRole("button", { name: "恢复默认" }));
    expect(input.value).toBe("43.240.223.138/u3400");
  });

  it("register + setValue 编程式改值后 DOM 跟随更新", async () => {
    function SetValueHarness() {
      const { register, setValue } = useForm<FormValues>({
        defaultValues: { endpoint: "10.0.0.9/u3400" },
      });
      return (
        <form>
          <Input data-testid="endpoint" {...register("endpoint")} />
          <button
            type="button"
            onClick={() => setValue("endpoint", "121.196.193.177/u3403")}
          >
            setValue
          </button>
        </form>
      );
    }
    render(<SetValueHarness />);
    const input = screen.getByTestId("endpoint") as HTMLInputElement;
    expect(input.value).toBe("10.0.0.9/u3400");
    fireEvent.click(screen.getByRole("button", { name: "setValue" }));
    await waitFor(() =>
      expect(input.value).toBe("121.196.193.177/u3403"),
    );
  });
});
