Feature: The monitoring routes should be responsive.

	Rule: The heartbeat route should always succeed.

		Example: Requesting the heartbeat route should succeed.
			Given a request is sent to the heartbeat route
			Then the request should succeed
