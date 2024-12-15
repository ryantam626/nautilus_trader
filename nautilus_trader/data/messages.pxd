# -------------------------------------------------------------------------------------------------
#  Copyright (C) 2015-2024 Nautech Systems Pty Ltd. All rights reserved.
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

from nautilus_trader.core.message cimport Command
from nautilus_trader.core.message cimport Request
from nautilus_trader.core.message cimport Response
from nautilus_trader.model.data cimport DataType
from nautilus_trader.model.identifiers cimport ClientId
from nautilus_trader.model.identifiers cimport Venue


cdef class DataCommand(Command):
    cdef readonly ClientId client_id
    """The data client ID for the command.\n\n:returns: `ClientId` or ``None``"""
    cdef readonly Venue venue
    """The venue for the command.\n\n:returns: `Venue` or ``None``"""
    cdef readonly DataType data_type
    """The command data type.\n\n:returns: `type`"""
    cdef readonly dict[str, object] params
    """Additional specific parameters for the command.\n\n:returns: `dict[str, object]` or ``None``"""


cdef class Subscribe(DataCommand):
    pass


cdef class Unsubscribe(DataCommand):
    pass


cdef class DataRequest(Request):
    cdef readonly ClientId client_id
    """The data client ID for the request.\n\n:returns: `ClientId` or ``None``"""
    cdef readonly Venue venue
    """The venue for the request.\n\n:returns: `Venue` or ``None``"""
    cdef readonly DataType data_type
    """The request data type.\n\n:returns: `type`"""
    cdef readonly dict[str, object] params
    """Additional specific parameters for the command.\n\n:returns: `dict[str, object]` or ``None``"""


cdef class DataResponse(Response):
    cdef readonly ClientId client_id
    """The data client ID for the response.\n\n:returns: `ClientId` or ``None``"""
    cdef readonly Venue venue
    """The venue for the response.\n\n:returns: `Venue` or ``None``"""
    cdef readonly DataType data_type
    """The response data type.\n\n:returns: `type`"""
    cdef readonly object data
    """The response data.\n\n:returns: `object`"""
    cdef readonly dict[str, object] params
    """Additional specific parameters for the response.\n\n:returns: `dict[str, object]` or ``None``"""


cdef inline str form_params_str(dict[str, object] params):
    return "" if not params else f", params={params}"
