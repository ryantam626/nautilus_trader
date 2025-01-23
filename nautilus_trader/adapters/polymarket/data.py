# -------------------------------------------------------------------------------------------------
#  Copyright (C) 2015-2025 Nautech Systems Pty Ltd. All rights reserved.
#  https://nautechsystems.io
#
#  Licensed under the GNU Lesser General Public License Version 3.0 (the "License");
#  You may not use this file except in compliance with the License.
#  You may obtain a copy of the License at https://www.gnu.org/licenses/lgpl-3.0.en.html
#
#  Unless required by applicable law or agreed to in writing, software
#  distributed under the License is distributed on an "AS IS" BASIS,
#  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
#  See the License for the specific language governing permissions and
#  limitations under the License.
# -------------------------------------------------------------------------------------------------

import asyncio
from collections.abc import Coroutine
from typing import Any

import msgspec
import pandas as pd
from py_clob_client.client import ClobClient

from nautilus_trader.adapters.polymarket.common.constants import POLYMARKET_VENUE
from nautilus_trader.adapters.polymarket.common.deltas import compute_effective_deltas
from nautilus_trader.adapters.polymarket.common.symbol import get_polymarket_instrument_id
from nautilus_trader.adapters.polymarket.common.symbol import get_polymarket_token_id
from nautilus_trader.adapters.polymarket.config import PolymarketDataClientConfig
from nautilus_trader.adapters.polymarket.providers import PolymarketInstrumentProvider
from nautilus_trader.adapters.polymarket.schemas.book import PolymarketBookSnapshot
from nautilus_trader.adapters.polymarket.schemas.book import PolymarketQuotes
from nautilus_trader.adapters.polymarket.schemas.book import PolymarketTrade
from nautilus_trader.adapters.polymarket.websocket.client import PolymarketWebSocketChannel
from nautilus_trader.adapters.polymarket.websocket.client import PolymarketWebSocketClient
from nautilus_trader.adapters.polymarket.websocket.types import MARKET_WS_MESSAGE
from nautilus_trader.cache.cache import Cache
from nautilus_trader.common.component import LiveClock
from nautilus_trader.common.component import MessageBus
from nautilus_trader.common.enums import LogColor
from nautilus_trader.core.uuid import UUID4
from nautilus_trader.live.data_client import LiveMarketDataClient
from nautilus_trader.model.book import OrderBook
from nautilus_trader.model.data import BarType
from nautilus_trader.model.data import OrderBookDeltas
from nautilus_trader.model.data import QuoteTick
from nautilus_trader.model.enums import BookType
from nautilus_trader.model.identifiers import ClientId
from nautilus_trader.model.identifiers import InstrumentId
from nautilus_trader.model.identifiers import Venue
from nautilus_trader.model.instruments import BinaryOption


class PolymarketDataClient(LiveMarketDataClient):
    """
    Provides a data client for Polymarket, a decentralized predication market.

    Parameters
    ----------
    loop : asyncio.AbstractEventLoop
        The event loop for the client.
    http_client : py_clob_client.client.ClobClient
        The Polymarket HTTP client.
    msgbus : MessageBus
        The message bus for the client.
    cache : Cache
        The cache for the client.
    clock : LiveClock
        The clock for the client.
    instrument_provider : PolymarketInstrumentProvider
        The instrument provider.
    config : PolymarketDataClientConfig
        The configuration for the client.
    name : str, optional
        The custom client ID.

    """

    def __init__(
        self,
        loop: asyncio.AbstractEventLoop,
        http_client: ClobClient,
        msgbus: MessageBus,
        cache: Cache,
        clock: LiveClock,
        instrument_provider: PolymarketInstrumentProvider,
        config: PolymarketDataClientConfig,
        name: str | None,
    ) -> None:
        super().__init__(
            loop=loop,
            client_id=ClientId(name or POLYMARKET_VENUE.value),
            venue=POLYMARKET_VENUE,
            msgbus=msgbus,
            cache=cache,
            clock=clock,
            instrument_provider=instrument_provider,
        )

        # Configuration
        self._config = config
        self._log.info(f"{config.ws_connection_initial_delay_secs=}", LogColor.BLUE)
        self._log.info(f"{config.ws_connection_delay_secs=}", LogColor.BLUE)
        self._log.info(f"{config.update_instruments_interval_mins=}", LogColor.BLUE)
        self._log.info(f"{config.compute_effective_deltas=}", LogColor.BLUE)

        # HTTP API
        self._http_client = http_client

        # WebSocket API
        self._ws_base_url = self._config.base_url_ws
        self._ws_clients: list[PolymarketWebSocketClient] = []
        self._ws_client_pending_connection: PolymarketWebSocketClient | None = None

        self._decoder_market_msg = msgspec.json.Decoder(MARKET_WS_MESSAGE)

        # Tasks
        self._update_instruments_interval_mins: int | None = config.update_instruments_interval_mins
        self._update_instruments_task: asyncio.Task | None = None
        self._delayed_ws_client_connection_task: asyncio.Task | None = None

        # Hot caches
        self._last_quotes: dict[InstrumentId, QuoteTick] = {}
        self._local_books: dict[InstrumentId, OrderBook] = {}

    async def _connect(self) -> None:
        self._log.info("Initializing instruments...")
        await self._instrument_provider.initialize()
        self._send_all_instruments_to_data_engine()

        if self._update_instruments_interval_mins:
            self._update_instruments_task = self.create_task(
                self._update_instruments(self._update_instruments_interval_mins),
            )

    async def _disconnect(self) -> None:
        if self._update_instruments_task:
            self._log.debug("Canceling task 'update_instruments'")
            self._update_instruments_task.cancel()
            self._update_instruments_task = None

        if self._delayed_ws_client_connection_task:
            self._log.debug("Canceling task 'delayed_ws_client_connection'")
            self._delayed_ws_client_connection_task.cancel()
            self._delayed_ws_client_connection_task = None

        # Shutdown websockets
        tasks: set[Coroutine[Any, Any, None]] = set()

        for ws_client in self._ws_clients:
            if ws_client.is_connected():
                tasks.add(ws_client.disconnect())

        if tasks:
            await asyncio.gather(*tasks)

    def _create_websocket_client(self) -> PolymarketWebSocketClient:
        self._log.info("Creating new PolymarketWebSocketClient", LogColor.MAGENTA)
        return PolymarketWebSocketClient(
            self._clock,
            base_url=self._ws_base_url,
            channel=PolymarketWebSocketChannel.MARKET,
            handler=self._handle_ws_message,
            handler_reconnect=None,
            loop=self._loop,
        )

    def _send_all_instruments_to_data_engine(self) -> None:
        for instrument in self._instrument_provider.get_all().values():
            self._handle_data(instrument)

        for currency in self._instrument_provider.currencies().values():
            self._cache.add_currency(currency)

    async def _update_instruments(self, interval_mins: int) -> None:
        try:
            while True:
                self._log.debug(
                    f"Scheduled task 'update_instruments' to run in {interval_mins} minutes",
                )
                await asyncio.sleep(interval_mins * 60)
                await self._instrument_provider.initialize(reload=True)
                self._send_all_instruments_to_data_engine()
        except asyncio.CancelledError:
            self._log.debug("Canceled task 'update_instruments'")

    async def _delayed_ws_client_connection(
        self,
        ws_client: PolymarketWebSocketClient,
        sleep_secs: int,
    ) -> None:
        try:
            await asyncio.sleep(sleep_secs)
            self._ws_clients.append(ws_client)
            await ws_client.connect()
        finally:
            self._ws_client_pending_connection = None
            self._delayed_ws_client_connection_task = None

    async def _subscribe_asset_book(self, instrument_id):
        create_connect_task = False
        if self._ws_client_pending_connection is None:
            self._ws_client_pending_connection = self._create_websocket_client()
            create_connect_task = True

        token_id = get_polymarket_token_id(instrument_id)
        self._ws_client_pending_connection.subscribe_book(token_id)

        if create_connect_task:
            self._delayed_ws_client_connection_task = self.create_task(
                self._delayed_ws_client_connection(
                    self._ws_client_pending_connection,
                    (
                        self._config.ws_connection_delay_secs
                        if self._ws_clients
                        else self._config.ws_connection_initial_delay_secs
                    ),
                ),
                log_msg="Delayed start PolymarketWebSocketClient connection",
                success_msg="Finished delaying start of PolymarketWebSocketClient connection",
            )

    async def _subscribe_order_book_deltas(
        self,
        instrument_id: InstrumentId,
        book_type: BookType,
        depth: int | None = None,
        params: dict[str, Any] | None = None,
    ) -> None:
        if book_type == BookType.L3_MBO:
            self._log.error(
                "Cannot subscribe to order book deltas: "
                "L3_MBO data is not published by Polymarket. "
                "Valid book types are L1_MBP, L2_MBP",
            )
            return

        if self._config.compute_effective_deltas:
            local_book = OrderBook(instrument_id, book_type=BookType.L2_MBP)
            self._local_books[instrument_id] = local_book

        await self._subscribe_asset_book(instrument_id)

    async def _subscribe_quote_ticks(
        self,
        instrument_id: InstrumentId,
        params: dict[str, Any] | None = None,
    ) -> None:
        await self._subscribe_asset_book(instrument_id)

    async def _subscribe_trade_ticks(
        self,
        instrument_id: InstrumentId,
        params: dict[str, Any] | None = None,
    ) -> None:
        await self._subscribe_asset_book(instrument_id)

    async def _subscribe_bars(
        self,
        bar_type: BarType,
        params: dict[str, Any] | None = None,
    ) -> None:
        self._log.error(
            f"Cannot subscribe to {bar_type} bars: not implemented for Polymarket",
        )

    async def _unsubscribe_order_book_deltas(
        self,
        instrument_id: InstrumentId,
        params: dict[str, Any] | None = None,
    ) -> None:
        self._log.error(
            f"Cannot unsubscribe from {instrument_id} order book deltas: unsubscribing not supported by Polymarket",
        )

    async def _unsubscribe_order_book_snapshots(
        self,
        instrument_id: InstrumentId,
        params: dict[str, Any] | None = None,
    ) -> None:
        self._log.error(
            f"Cannot unsubscribe from {instrument_id} order book snapshots: unsubscribing not supported by Polymarket",
        )

    async def _unsubscribe_quote_ticks(
        self,
        instrument_id: InstrumentId,
        params: dict[str, Any] | None = None,
    ) -> None:
        self._log.error(
            f"Cannot unsubscribe from {instrument_id} quotes: unsubscribing not supported by Polymarket",
        )

    async def _unsubscribe_trade_ticks(
        self,
        instrument_id: InstrumentId,
        params: dict[str, Any] | None = None,
    ) -> None:
        self._log.error(
            f"Cannot unsubscribe from {instrument_id} trades: unsubscribing not supported by Polymarket",
        )

    async def _unsubscribe_bars(
        self,
        bar_type: BarType,
        params: dict[str, Any] | None = None,
    ) -> None:
        self._log.error(
            f"Cannot unsubscribe from {bar_type} bars: not implemented for Polymarket",
        )

    async def _request_instrument(
        self,
        instrument_id: InstrumentId,
        correlation_id: UUID4,
        start: pd.Timestamp | None = None,
        end: pd.Timestamp | None = None,
        params: dict[str, Any] | None = None,
    ) -> None:
        if start is not None:
            self._log.warning(
                f"Requesting instrument {instrument_id} with specified `start` which has no effect",
            )

        if end is not None:
            self._log.warning(
                f"Requesting instrument {instrument_id} with specified `end` which has no effect",
            )

        instrument: BinaryOption | None = self._instrument_provider.find(instrument_id)
        if instrument is None:
            self._log.error(f"Cannot find instrument for {instrument_id}")
            return

        self._handle_instrument(instrument, correlation_id, params)

    async def _request_instruments(
        self,
        venue: Venue,
        correlation_id: UUID4,
        start: pd.Timestamp | None = None,
        end: pd.Timestamp | None = None,
        params: dict[str, Any] | None = None,
    ) -> None:
        if start is not None:
            self._log.warning(
                f"Requesting instruments for {venue} with specified `start` which has no effect",
            )

        if end is not None:
            self._log.warning(
                f"Requesting instruments for {venue} with specified `end` which has no effect",
            )

        all_instruments = self._instrument_provider.get_all()
        target_instruments = []
        for instrument in all_instruments.values():
            if instrument.venue == venue:
                target_instruments.append(instrument)

        self._handle_instruments(target_instruments, venue, correlation_id, params)

    async def _request_quote_ticks(
        self,
        instrument_id: InstrumentId,
        limit: int,
        correlation_id: UUID4,
        start: pd.Timestamp | None = None,
        end: pd.Timestamp | None = None,
        params: dict[str, Any] | None = None,
    ) -> None:
        self._log.error("Cannot request historical quotes: not published by Polymarket")

    async def _request_trade_ticks(
        self,
        instrument_id: InstrumentId,
        limit: int,
        correlation_id: UUID4,
        start: pd.Timestamp | None = None,
        end: pd.Timestamp | None = None,
        params: dict[str, Any] | None = None,
    ) -> None:
        self._log.error("Cannot request historical trades: not published by Polymarket")

    async def _request_bars(
        self,
        bar_type: BarType,
        limit: int,
        correlation_id: UUID4,
        start: pd.Timestamp | None = None,
        end: pd.Timestamp | None = None,
        params: dict[str, Any] | None = None,
    ) -> None:
        self._log.error("Cannot request historical bars: not published by Polymarket")

    def _handle_ws_message(self, raw: bytes) -> None:
        # Uncomment for development
        # self._log.info(str(raw), LogColor.MAGENTA)
        try:
            ws_message = self._decoder_market_msg.decode(raw)
            for msg in ws_message:
                if isinstance(msg, list):
                    if isinstance(msg, PolymarketBookSnapshot):
                        instrument_id = get_polymarket_instrument_id(msg.market, msg.asset_id)
                        instrument = self._cache.instrument(instrument_id)
                        if instrument is None:
                            self._log.error(f"Cannot find instrument for {instrument_id}")
                            return
                        self._handle_book_snapshot(instrument=instrument, ws_message=msg)
                else:
                    instrument_id = get_polymarket_instrument_id(msg.market, msg.asset_id)
                    instrument = self._cache.instrument(instrument_id)
                    if instrument is None:
                        self._log.error(f"Cannot find instrument for {instrument_id}")
                        return

                    if isinstance(msg, PolymarketBookSnapshot):
                        self._handle_book_snapshot(instrument=instrument, ws_message=msg)
                    elif isinstance(msg, PolymarketQuotes):
                        self._handle_quote(instrument=instrument, ws_message=msg)
                    elif isinstance(msg, PolymarketTrade):
                        self._handle_trade(instrument=instrument, ws_message=msg)
                    else:
                        self._log.error(f"Unknown websocket message topic: {ws_message}")
        except Exception as e:
            self._handle_ws_message_failure(e, raw)

    def _handle_ws_message_failure(self, exception: Exception, raw: bytes) -> None:
        decoded = raw.decode()
        if decoded == "PONG":
            self._log.info("Got PONG")
        else:
            self._log.error(f"Failed to parse websocket message: {decoded} with error {exception}")

    def _handle_book_snapshot(
        self,
        instrument: BinaryOption,
        ws_message: PolymarketBookSnapshot,
    ) -> None:
        now_ns = self._clock.timestamp_ns()
        deltas = ws_message.parse_to_snapshot(instrument=instrument, ts_init=now_ns)

        self._handle_deltas(instrument, deltas)

        if instrument.id in self.subscribed_quote_ticks():
            quote = ws_message.parse_to_quote_tick(instrument=instrument, ts_init=now_ns)
            self._last_quotes[instrument.id] = quote
            self._handle_data(quote)

    def _handle_deltas(self, instrument: BinaryOption, deltas: OrderBookDeltas) -> None:
        if self._config.compute_effective_deltas:
            # Compute effective deltas (reduce snapshot based on old and new book states),
            # prioritizing a smaller data footprint over computational efficiency.
            t0 = self._clock.timestamp_ns()
            book_old = self._local_books.get(instrument.id)
            book_new = OrderBook(instrument.id, book_type=BookType.L2_MBP)
            book_new.apply_deltas(deltas)
            self._local_books[instrument.id] = book_new
            deltas = compute_effective_deltas(book_old, book_new, instrument)

            interval_ms = (self._clock.timestamp_ns() - t0) / 1_000_000
            self._log.debug(f"Computed effective deltas in {interval_ms:.3f}ms")
            # self._log.warning(book_new.pprint())  # Uncomment for development

        # Check if any effective deltas remain
        if deltas:
            self._handle_data(deltas)

    def _handle_quote(
        self,
        instrument: BinaryOption,
        ws_message: PolymarketQuotes,
    ) -> None:
        now_ns = self._clock.timestamp_ns()
        deltas = ws_message.parse_to_deltas(instrument=instrument, ts_init=now_ns)

        if self._config.compute_effective_deltas:
            local_book = self._local_books.get(instrument.id)
            if local_book:
                local_book.apply_deltas(deltas)

        self._handle_data(deltas)

        if instrument.id in self.subscribed_quote_ticks():
            last_quote = self._last_quotes.get(instrument.id)
            if last_quote is None:
                return

            # Check if top-of-book change
            quotes = ws_message.parse_to_quote_ticks(
                instrument=instrument,
                last_quote=last_quote,
                ts_init=self._clock.timestamp_ns(),
            )
            for quote in quotes:
                if (
                    quote.bid_price == last_quote.bid_price
                    and quote.ask_price == last_quote.ask_price
                    and quote.bid_size == last_quote.bid_size
                    and quote.ask_size == last_quote.ask_size
                ):
                    continue  # No top-of-book change

                self._last_quotes[instrument.id] = quote
                self._handle_data(quote)

    def _handle_trade(
        self,
        instrument: BinaryOption,
        ws_message: PolymarketTrade,
    ) -> None:
        now_ns = self._clock.timestamp_ns()
        trade = ws_message.parse_to_trade_tick(instrument=instrument, ts_init=now_ns)
        self._handle_data(trade)
