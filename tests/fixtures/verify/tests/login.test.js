it("rejects an expired token", async () => {
  expect(check("expired")).toBe(false);
});
