// -------------------------------------------------------------------------------------------------
//  Copyright (C) 2015-2024 Nautech Systems Pty Ltd. All rights reserved.
//  https://nautechsystems.io
//
//  Licensed under the GNU Lesser General Public License Version 3.0 (the "License");
//  You may not use this file except in compliance with the License.
//  You may obtain a copy of the License at https://www.gnu.org/licenses/lgpl-3.0.en.html
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.
// -------------------------------------------------------------------------------------------------

use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use nautilus_core::{nanos::UnixNanos, uuid::UUID4};
use serde::{Deserialize, Serialize};
use ustr::Ustr;

use super::{
    any::OrderAny,
    base::{Order, OrderCore, OrderError},
};
use crate::{
    enums::{
        ContingencyType, LiquiditySide, OrderSide, OrderStatus, OrderType, TimeInForce,
        TrailingOffsetType, TriggerType,
    },
    events::{OrderEventAny, OrderInitialized, OrderUpdated},
    identifiers::{
        AccountId, ClientOrderId, ExecAlgorithmId, InstrumentId, OrderListId, PositionId,
        StrategyId, Symbol, TradeId, TraderId, Venue, VenueOrderId,
    },
    types::{Price, Quantity},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(
    feature = "python",
    pyo3::pyclass(module = "nautilus_trader.core.nautilus_pyo3.model")
)]
pub struct MarketIfTouchedOrder {
    pub trigger_price: Price,
    pub trigger_type: TriggerType,
    pub expire_time: Option<UnixNanos>,
    pub display_qty: Option<Quantity>,
    pub trigger_instrument_id: Option<InstrumentId>,
    pub is_triggered: bool,
    pub ts_triggered: Option<UnixNanos>,
    core: OrderCore,
}

impl MarketIfTouchedOrder {
    /// Creates a new [`MarketIfTouchedOrder`] instance.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        trader_id: TraderId,
        strategy_id: StrategyId,
        instrument_id: InstrumentId,
        client_order_id: ClientOrderId,
        order_side: OrderSide,
        quantity: Quantity,
        trigger_price: Price,
        trigger_type: TriggerType,
        time_in_force: TimeInForce,
        expire_time: Option<UnixNanos>,
        reduce_only: bool,
        quote_quantity: bool,
        display_qty: Option<Quantity>,
        emulation_trigger: Option<TriggerType>,
        trigger_instrument_id: Option<InstrumentId>,
        contingency_type: Option<ContingencyType>,
        order_list_id: Option<OrderListId>,
        linked_order_ids: Option<Vec<ClientOrderId>>,
        parent_order_id: Option<ClientOrderId>,
        exec_algorithm_id: Option<ExecAlgorithmId>,
        exec_algorithm_params: Option<HashMap<Ustr, Ustr>>,
        exec_spawn_id: Option<ClientOrderId>,
        tags: Option<Vec<Ustr>>,
        init_id: UUID4,
        ts_init: UnixNanos,
    ) -> Self {
        let init_order = OrderInitialized::new(
            trader_id,
            strategy_id,
            instrument_id,
            client_order_id,
            order_side,
            OrderType::MarketIfTouched,
            quantity,
            time_in_force,
            false,
            reduce_only,
            quote_quantity,
            false,
            init_id,
            ts_init,
            ts_init,
            None,
            Some(trigger_price),
            Some(trigger_type),
            None,
            None,
            None,
            expire_time,
            display_qty,
            emulation_trigger,
            trigger_instrument_id,
            contingency_type,
            order_list_id,
            linked_order_ids,
            parent_order_id,
            exec_algorithm_id,
            exec_algorithm_params,
            exec_spawn_id,
            tags,
        );
        Self {
            core: OrderCore::new(init_order),
            trigger_price,
            trigger_type,
            expire_time,
            display_qty,
            trigger_instrument_id,
            is_triggered: false,
            ts_triggered: None,
        }
    }
}

impl Deref for MarketIfTouchedOrder {
    type Target = OrderCore;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl DerefMut for MarketIfTouchedOrder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}

impl Order for MarketIfTouchedOrder {
    fn into_any(self) -> OrderAny {
        OrderAny::MarketIfTouched(self)
    }

    fn status(&self) -> OrderStatus {
        self.status
    }

    fn trader_id(&self) -> TraderId {
        self.trader_id
    }

    fn strategy_id(&self) -> StrategyId {
        self.strategy_id
    }

    fn instrument_id(&self) -> InstrumentId {
        self.instrument_id
    }

    fn symbol(&self) -> Symbol {
        self.instrument_id.symbol
    }

    fn venue(&self) -> Venue {
        self.instrument_id.venue
    }

    fn client_order_id(&self) -> ClientOrderId {
        self.client_order_id
    }

    fn venue_order_id(&self) -> Option<VenueOrderId> {
        self.venue_order_id
    }

    fn position_id(&self) -> Option<PositionId> {
        self.position_id
    }

    fn account_id(&self) -> Option<AccountId> {
        self.account_id
    }

    fn last_trade_id(&self) -> Option<TradeId> {
        self.last_trade_id
    }

    fn side(&self) -> OrderSide {
        self.side
    }

    fn order_type(&self) -> OrderType {
        self.order_type
    }

    fn quantity(&self) -> Quantity {
        self.quantity
    }

    fn time_in_force(&self) -> TimeInForce {
        self.time_in_force
    }

    fn expire_time(&self) -> Option<UnixNanos> {
        self.expire_time
    }

    fn price(&self) -> Option<Price> {
        None
    }

    fn trigger_price(&self) -> Option<Price> {
        Some(self.trigger_price)
    }

    fn trigger_type(&self) -> Option<TriggerType> {
        Some(self.trigger_type)
    }

    fn liquidity_side(&self) -> Option<LiquiditySide> {
        self.liquidity_side
    }

    fn is_post_only(&self) -> bool {
        false
    }

    fn is_reduce_only(&self) -> bool {
        self.is_reduce_only
    }

    fn is_quote_quantity(&self) -> bool {
        self.is_quote_quantity
    }

    fn display_qty(&self) -> Option<Quantity> {
        self.display_qty
    }

    fn limit_offset(&self) -> Option<Price> {
        None
    }

    fn trailing_offset(&self) -> Option<Price> {
        None
    }

    fn trailing_offset_type(&self) -> Option<TrailingOffsetType> {
        None
    }

    fn emulation_trigger(&self) -> Option<TriggerType> {
        self.emulation_trigger
    }

    fn trigger_instrument_id(&self) -> Option<InstrumentId> {
        self.trigger_instrument_id
    }

    fn contingency_type(&self) -> Option<ContingencyType> {
        self.contingency_type
    }

    fn order_list_id(&self) -> Option<OrderListId> {
        self.order_list_id
    }

    fn linked_order_ids(&self) -> Option<&[ClientOrderId]> {
        self.linked_order_ids.as_deref()
    }

    fn parent_order_id(&self) -> Option<ClientOrderId> {
        self.parent_order_id
    }

    fn exec_algorithm_id(&self) -> Option<ExecAlgorithmId> {
        self.exec_algorithm_id
    }

    fn exec_algorithm_params(&self) -> Option<&HashMap<Ustr, Ustr>> {
        self.exec_algorithm_params.as_ref()
    }

    fn exec_spawn_id(&self) -> Option<ClientOrderId> {
        self.exec_spawn_id
    }

    fn tags(&self) -> Option<&[Ustr]> {
        self.tags.as_deref()
    }

    fn filled_qty(&self) -> Quantity {
        self.filled_qty
    }

    fn leaves_qty(&self) -> Quantity {
        self.leaves_qty
    }

    fn avg_px(&self) -> Option<f64> {
        self.avg_px
    }

    fn slippage(&self) -> Option<f64> {
        self.slippage
    }

    fn init_id(&self) -> UUID4 {
        self.init_id
    }

    fn ts_init(&self) -> UnixNanos {
        self.ts_init
    }

    fn ts_last(&self) -> UnixNanos {
        self.ts_last
    }

    fn events(&self) -> Vec<&OrderEventAny> {
        self.events.iter().collect()
    }

    fn venue_order_ids(&self) -> Vec<&VenueOrderId> {
        self.venue_order_ids.iter().collect()
    }

    fn trade_ids(&self) -> Vec<&TradeId> {
        self.trade_ids.iter().collect()
    }

    fn apply(&mut self, event: OrderEventAny) -> Result<(), OrderError> {
        if let OrderEventAny::Updated(ref event) = event {
            self.update(event);
        };
        let is_order_filled = matches!(event, OrderEventAny::Filled(_));

        self.core.apply(event)?;

        if is_order_filled {
            self.core.set_slippage(self.trigger_price);
        };

        Ok(())
    }

    fn update(&mut self, event: &OrderUpdated) {
        assert!(event.price.is_none(), "{}", OrderError::InvalidOrderEvent);

        if let Some(trigger_price) = event.trigger_price {
            self.trigger_price = trigger_price;
        }

        self.quantity = event.quantity;
        self.leaves_qty = self.quantity - self.filled_qty;
    }
}

impl From<OrderAny> for MarketIfTouchedOrder {
    fn from(order: OrderAny) -> MarketIfTouchedOrder {
        match order {
            OrderAny::MarketIfTouched(order) => order,
            _ => {
                panic!(
                    "Invalid `OrderAny` not `{}`, was {order:?}",
                    stringify!(MarketIfTouchedOrder),
                )
            }
        }
    }
}

impl From<OrderInitialized> for MarketIfTouchedOrder {
    fn from(event: OrderInitialized) -> Self {
        Self::new(
            event.trader_id,
            event.strategy_id,
            event.instrument_id,
            event.client_order_id,
            event.order_side,
            event.quantity,
            event
                .trigger_price // TODO: Improve this error, model order domain errors
                .expect(
                    "Error initializing order: `trigger_price` was `None` for `MarketIfTouchedOrder`",
                ),
            event
                .trigger_type
                .expect("Error initializing order: `trigger_type` was `None` for `MarketIfTouchedOrder`"),
            event.time_in_force,
            event.expire_time,
            event.reduce_only,
            event.quote_quantity,
            event.display_qty,
            event.emulation_trigger,
            event.trigger_instrument_id,
            event.contingency_type,
            event.order_list_id,
            event.linked_order_ids,
            event.parent_order_id,
            event.exec_algorithm_id,
            event.exec_algorithm_params,
            event.exec_spawn_id,
            event.tags,
            event.event_id,
            event.ts_event,
        )
    }
}
