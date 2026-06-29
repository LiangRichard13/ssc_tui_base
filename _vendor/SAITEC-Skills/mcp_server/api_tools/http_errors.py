"""HTTP error helpers for SAITEC skill API calls."""

import httpx


def raise_for_status_with_body(response: httpx.Response) -> None:
    if response.is_success:
        return

    message = (
        f"{response.status_code} {response.reason_phrase} for url "
        f"'{response.request.url}'"
    )
    body = response.text.strip()
    if body:
        message = f"{message}: {body}"

    raise httpx.HTTPStatusError(message, request=response.request, response=response)
