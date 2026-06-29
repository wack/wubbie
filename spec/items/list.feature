Feature: Listing items.

	Rule: The items list endpoint should respond successfully.

		Example: Listing items against the mock-backed app succeeds.
			When the client lists items
			Then the response should be successful
